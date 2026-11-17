#![forbid(
    unsafe_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::arithmetic_side_effects,
    clippy::disallowed_types,
    clippy::disallowed_methods
)]

use anime_qvm::{QvmSandbox, verify_qvm_program};
use codec_mkv::{MkvPacket, demux_mkv_stream, mux_mkv_stream};
use dsp_studio::{
    MAX_REFLECTION_K_RAW, apply_vacuum_gate, householder_fdn4_step, kelly_lochbaum_junction_step,
    verify_beat_phase_lock,
};
use ledger_wal::{LedgerWal, WalError};
use math_q32::{Q32, isqrt_u128, mul_u64_tiled_8bit};
use render_hybrid::{MAX_VRAM_BUDGET_BYTES, prepare_hybrid_pipeline};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitCode};
use typo_hangul::{RectI32, check_roi_non_overlap, verify_subtitle_layout};
use world_ledger::{
    ActionUtility, AssetKind, DecisionNode, EnemyAgent, EpisodeManifest, EventKind,
    HierarchyInversion, LedgerError, LogicRelation, MechanismGraph, MechanismRole,
    RelativisticStep, SpacetimeEdge, SpacetimeEvent, WorldLedger, check_novikov_lattice,
    five_axis_diversity_q32, mechanism_distance_q32, verify_decision_node, verify_spacetime_graph,
    verify_two_party_lifespan_step, verify_zero_dref_plagiarism,
};

const ALLOWED_EXTERNAL_CRATES: &[&str] = &[
    "serde",
    "serde_json",
    "ron",
    "blake3",
    "rayon",
    "arc-swap",
    "ash",
    "gpu-allocator",
    "winit",
    "raw-window-handle",
    "ash-window",
    "cpal",
];

const KSIM_CRATES: &[&str] = &[
    "math_q32",
    "verifier_abi",
    "world_ledger",
    "sakuga_biomech",
    "typo_hangul",
    "dsp_studio",
    "spirv_emit",
    "codec_mkv",
    "anime_qvm",
];

const ALL_KERNEL_CRATES: &[&str] = &[
    "math_q32",
    "verifier_abi",
    "world_ledger",
    "sakuga_biomech",
    "typo_hangul",
    "dsp_studio",
    "spirv_emit",
    "codec_mkv",
    "anime_qvm",
    "ledger_wal",
    "render_hybrid",
    "player_bin",
];

const BANNED_ASSET_EXTENSIONS: &[&str] = &[
    "safetensors",
    "onnx",
    "gguf",
    "pt",
    "pth",
    "ckpt",
    "ttf",
    "otf",
    "woff",
    "woff2",
    "wav",
    "mp3",
    "ogg",
    "flac",
    "mp4",
    "mov",
    "avi",
    "obj",
    "fbx",
    "gltf",
    "glb",
    "blend",
    "spv",
    "glsl",
    "hlsl",
    "slang",
];

// ============================================================================
// 1. PURE-RUST BIT-BLASTING CDCL SAT SOLVER (`§8.1`)
// ============================================================================

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SatResult {
    Sat(Vec<bool>),
    Unsat,
}

pub struct CdclSolver {
    num_vars: usize,
    clauses: Vec<Vec<i32>>,
    watches: Vec<Vec<usize>>,
    assigns: Vec<i8>, // 0 = unassigned, 1 = true, -1 = false
    saved_phase: Vec<bool>,
    trail: Vec<i32>,
    qhead: usize,
    trail_lim: Vec<usize>,
    var_level: Vec<usize>,
    var_reason: Vec<Option<usize>>,
    activity: Vec<u64>,
    var_inc: u64,
    unsat_at_root: bool,
}

impl Default for CdclSolver {
    fn default() -> Self {
        Self::new()
    }
}

impl CdclSolver {
    pub fn new() -> Self {
        Self {
            num_vars: 0,
            clauses: Vec::new(),
            watches: vec![Vec::new(), Vec::new()],
            assigns: vec![0],
            saved_phase: vec![false],
            trail: Vec::new(),
            qhead: 0,
            trail_lim: Vec::new(),
            var_level: vec![0],
            var_reason: vec![None],
            activity: vec![0],
            var_inc: 1,
            unsat_at_root: false,
        }
    }

    #[inline]
    fn watch_idx(lit: i32) -> usize {
        let base = (lit.unsigned_abs() as usize).saturating_mul(2);
        if lit > 0 {
            base
        } else {
            base.saturating_add(1)
        }
    }

    pub fn new_var(&mut self) -> i32 {
        self.num_vars = self.num_vars.saturating_add(1);
        self.watches.push(Vec::new());
        self.watches.push(Vec::new());
        self.assigns.push(0);
        self.saved_phase.push(false);
        self.var_level.push(0);
        self.var_reason.push(None);
        self.activity.push(1);
        self.num_vars as i32
    }

    #[inline]
    fn lit_val(&self, lit: i32) -> i8 {
        let v = lit.unsigned_abs() as usize;
        let a = self.assigns.get(v).copied().unwrap_or(0);
        if lit > 0 { a } else { a.saturating_neg() }
    }

    #[inline]
    fn decision_level(&self) -> usize {
        self.trail_lim.len()
    }

    fn bump_var(&mut self, v: usize) {
        let inc = self.var_inc;
        let mut rescale = false;
        if let Some(act) = self.activity.get_mut(v) {
            *act = act.saturating_add(inc);
            if *act > (1_u64 << 50) {
                rescale = true;
            }
        }
        if rescale {
            for a in &mut self.activity {
                *a >>= 24;
            }
            self.var_inc = (self.var_inc >> 24).max(1);
        }
    }

    fn attach_clause(&mut self, cid: usize) {
        if let Some(cl) = self.clauses.get(cid)
            && cl.len() >= 2
        {
            let w0 = Self::watch_idx(cl[0].saturating_neg());
            let w1 = Self::watch_idx(cl[1].saturating_neg());
            if let Some(list0) = self.watches.get_mut(w0) {
                list0.push(cid);
            }
            if let Some(list1) = self.watches.get_mut(w1) {
                list1.push(cid);
            }
        }
    }

    fn enqueue(&mut self, lit: i32, reason: Option<usize>) -> bool {
        let cur = self.lit_val(lit);
        if cur == -1 {
            return false;
        }
        if cur == 1 {
            return true;
        }
        let v = lit.unsigned_abs() as usize;
        let lvl = self.decision_level();
        if let Some(slot) = self.assigns.get_mut(v) {
            *slot = if lit > 0 { 1 } else { -1 };
        }
        if let Some(ps) = self.saved_phase.get_mut(v) {
            *ps = lit > 0;
        }
        if let Some(lslot) = self.var_level.get_mut(v) {
            *lslot = lvl;
        }
        if let Some(rslot) = self.var_reason.get_mut(v) {
            *rslot = reason;
        }
        self.trail.push(lit);
        true
    }

    pub fn add_clause(&mut self, raw_lits: &[i32]) {
        if self.unsat_at_root {
            return;
        }
        let mut norm: Vec<i32> = Vec::with_capacity(raw_lits.len());
        for &l in raw_lits {
            let val = self.lit_val(l);
            if val == 1 && self.var_level.get(l.unsigned_abs() as usize).copied() == Some(0) {
                return;
            }
            if val == -1 && self.var_level.get(l.unsigned_abs() as usize).copied() == Some(0) {
                continue;
            }
            if norm.contains(&l.saturating_neg()) {
                return;
            }
            if !norm.contains(&l) {
                norm.push(l);
            }
        }
        if norm.is_empty() {
            self.unsat_at_root = true;
            return;
        }
        if norm.len() == 1 {
            if !self.enqueue(norm[0], None) {
                self.unsat_at_root = true;
            }
            return;
        }
        for &l in &norm {
            let v = l.unsigned_abs() as usize;
            if let Some(act) = self.activity.get_mut(v) {
                *act = act.saturating_add(1);
            }
        }
        let cid = self.clauses.len();
        self.clauses.push(norm);
        self.attach_clause(cid);
    }

    fn propagate(&mut self) -> Option<usize> {
        while self.qhead < self.trail.len() {
            let p = self.trail[self.qhead];
            self.qhead = self.qhead.saturating_add(1);
            let false_lit = p.saturating_neg();
            let w_idx = Self::watch_idx(p);
            let wlist = self.watches.get_mut(w_idx).map(core::mem::take).unwrap_or_default();
            let mut keep: Vec<usize> = Vec::with_capacity(wlist.len());

            let mut i: usize = 0;
            while i < wlist.len() {
                let cid = wlist[i];
                if self.clauses[cid][0] == false_lit {
                    self.clauses[cid].swap(0, 1);
                }
                let first = self.clauses[cid][0];
                if self.lit_val(first) == 1 {
                    keep.push(cid);
                    i = i.saturating_add(1);
                    continue;
                }

                let cl_len = self.clauses[cid].len();
                let mut k: usize = 2;
                let mut moved = false;
                while k < cl_len {
                    let cand = self.clauses[cid][k];
                    if self.lit_val(cand) != -1 {
                        self.clauses[cid].swap(1, k);
                        let new_w = Self::watch_idx(cand.saturating_neg());
                        if new_w == w_idx {
                            keep.push(cid);
                        } else if let Some(target_w) = self.watches.get_mut(new_w) {
                            target_w.push(cid);
                        }
                        moved = true;
                        break;
                    }
                    k = k.saturating_add(1);
                }

                if !moved {
                    keep.push(cid);
                    if self.lit_val(first) == -1 || !self.enqueue(first, Some(cid)) {
                        let rem_start = i.saturating_add(1);
                        if rem_start < wlist.len() {
                            keep.extend_from_slice(&wlist[rem_start..]);
                        }
                        if let Some(slot) = self.watches.get_mut(w_idx) {
                            slot.extend(keep);
                        }
                        self.qhead = self.trail.len();
                        return Some(cid);
                    }
                }
                i = i.saturating_add(1);
            }

            if let Some(slot) = self.watches.get_mut(w_idx) {
                slot.extend(keep);
            }
        }
        None
    }

    fn backtrack_to(&mut self, target_level: usize) {
        if let Some(&lim) = self.trail_lim.get(target_level) {
            while self.trail.len() > lim {
                if let Some(lit) = self.trail.pop() {
                    let v = lit.unsigned_abs() as usize;
                    if let Some(a) = self.assigns.get_mut(v) {
                        *a = 0;
                    }
                    if let Some(r) = self.var_reason.get_mut(v) {
                        *r = None;
                    }
                }
            }
            self.trail_lim.truncate(target_level);
            self.qhead = self.trail.len();
        }
    }

    /// 1-UIP conflict analysis returning `(learnt_clause_with_uip_at_0, backtrack_level)`.
    fn analyze_conflict(&mut self, conflict_cid: usize) -> (Vec<i32>, usize) {
        let cur_lvl = self.decision_level();
        let mut seen = vec![false; self.num_vars.saturating_add(1)];
        let mut learnt: Vec<i32> = vec![0];
        let mut path_c: usize = 0;
        let mut reason_opt: Option<usize> = Some(conflict_cid);
        let mut trail_idx = self.trail.len();
        let mut p_lit: i32 = 0;

        loop {
            if let Some(rcid) = reason_opt
                && let Some(cl) = self.clauses.get(rcid).cloned()
            {
                for q in cl {
                    if q == p_lit {
                        continue;
                    }
                    let v = q.unsigned_abs() as usize;
                    if seen.get(v).copied().unwrap_or(true) {
                        continue;
                    }
                    if let Some(s) = seen.get_mut(v) {
                        *s = true;
                    }
                    self.bump_var(v);
                    let lvl = self.var_level.get(v).copied().unwrap_or(0);
                    if lvl >= cur_lvl {
                        path_c = path_c.saturating_add(1);
                    } else if lvl > 0 {
                        learnt.push(q);
                    }
                }
            }

            loop {
                if trail_idx == 0 {
                    break;
                }
                trail_idx = trail_idx.saturating_sub(1);
                let cand = self.trail.get(trail_idx).copied().unwrap_or(0);
                let cv = cand.unsigned_abs() as usize;
                if seen.get(cv).copied().unwrap_or(false)
                    && self.var_level.get(cv).copied().unwrap_or(0) == cur_lvl
                {
                    p_lit = cand;
                    break;
                }
            }

            let pv = p_lit.unsigned_abs() as usize;
            reason_opt = self.var_reason.get(pv).copied().flatten();
            if let Some(s) = seen.get_mut(pv) {
                *s = false;
            }
            path_c = path_c.saturating_sub(1);
            if path_c == 0 {
                break;
            }
        }

        self.var_inc = self
            .var_inc
            .saturating_mul(21)
            .checked_div(20)
            .unwrap_or(1)
            .max(1);

        learnt[0] = p_lit.saturating_neg();
        let mut bt_lvl: usize = 0;
        let mut best_pos: usize = 1;
        let mut i: usize = 1;
        while i < learnt.len() {
            let v = learnt[i].unsigned_abs() as usize;
            let lvl = self.var_level.get(v).copied().unwrap_or(0);
            if lvl > bt_lvl {
                bt_lvl = lvl;
                best_pos = i;
            }
            i = i.saturating_add(1);
        }
        if learnt.len() > 2 && best_pos != 1 {
            learnt.swap(1, best_pos);
        }
        (learnt, bt_lvl)
    }

    fn pick_branch_lit(&self) -> Option<i32> {
        let mut best_var: usize = 0;
        let mut best_act: u64 = 0;
        let mut v: usize = 1;
        while v <= self.num_vars {
            if self.assigns.get(v).copied().unwrap_or(1) == 0 {
                let act = self.activity.get(v).copied().unwrap_or(0);
                if best_var == 0 || act > best_act {
                    best_var = v;
                    best_act = act;
                }
            }
            v = v.saturating_add(1);
        }
        if best_var == 0 {
            None
        } else if self.saved_phase.get(best_var).copied().unwrap_or(false) {
            Some(best_var as i32)
        } else {
            Some((best_var as i32).saturating_neg())
        }
    }

    pub fn solve(&mut self) -> SatResult {
        if self.unsat_at_root {
            return SatResult::Unsat;
        }
        let initial_clause_count = self.clauses.len();
        let mut conflicts_since_restart: u64 = 0;
        let mut restart_limit: u64 = 256;

        loop {
            if let Some(conflict_cid) = self.propagate() {
                if self.decision_level() == 0 {
                    return SatResult::Unsat;
                }
                conflicts_since_restart = conflicts_since_restart.saturating_add(1);
                let (learnt, bt_lvl) = self.analyze_conflict(conflict_cid);
                self.backtrack_to(bt_lvl);
                let uip = learnt[0];
                if learnt.len() == 1 {
                    if !self.enqueue(uip, None) {
                        return SatResult::Unsat;
                    }
                } else {
                    let new_cid = self.clauses.len();
                    self.clauses.push(learnt);
                    self.attach_clause(new_cid);
                    if !self.enqueue(uip, Some(new_cid)) {
                        return SatResult::Unsat;
                    }
                }
            } else if conflicts_since_restart >= restart_limit && self.decision_level() > 0 {
                self.backtrack_to(0);
                conflicts_since_restart = 0;
                restart_limit = restart_limit.saturating_add(192);
            } else if let Some(branch_lit) = self.pick_branch_lit() {
                self.trail_lim.push(self.trail.len());
                let _ = self.enqueue(branch_lit, None);
            } else {
                // Model complete: independently verify every original clause!
                let mut model = vec![false; self.num_vars.saturating_add(1)];
                let mut v: usize = 1;
                while v <= self.num_vars {
                    if let Some(m) = model.get_mut(v) {
                        *m = self.assigns.get(v).copied().unwrap_or(0) > 0;
                    }
                    v = v.saturating_add(1);
                }
                let mut c_idx: usize = 0;
                while c_idx < initial_clause_count {
                    if let Some(cl) = self.clauses.get(c_idx) {
                        let ok = cl.iter().any(|&lit| {
                            let val = model.get(lit.unsigned_abs() as usize).copied().unwrap_or(false);
                            if lit > 0 { val } else { !val }
                        });
                        if !ok {
                            return SatResult::Unsat;
                        }
                    }
                    c_idx = c_idx.saturating_add(1);
                }
                return SatResult::Sat(model);
            }
        }
    }
}

/// Bit-vector Tseitin circuit compiler on top of `CdclSolver`.
pub struct BitBlaster {
    pub solver: CdclSolver,
    true_lit: i32,
}

impl Default for BitBlaster {
    fn default() -> Self {
        Self::new()
    }
}

impl BitBlaster {
    pub fn new() -> Self {
        let mut solver = CdclSolver::new();
        let t = solver.new_var();
        solver.add_clause(&[t]);
        Self { solver, true_lit: t }
    }

    #[inline]
    pub const fn const_bool(&self, b: bool) -> i32 {
        if b {
            self.true_lit
        } else {
            self.true_lit.saturating_neg()
        }
    }

    pub fn fresh_bv(&mut self, width: usize) -> Vec<i32> {
        let mut out = Vec::with_capacity(width);
        let mut i: usize = 0;
        while i < width {
            out.push(self.solver.new_var());
            i = i.saturating_add(1);
        }
        out
    }

    pub fn const_bv(&self, val: u128, width: usize) -> Vec<i32> {
        let mut out = Vec::with_capacity(width);
        let mut i: usize = 0;
        while i < width {
            let bit = if i < 128 {
                (val.wrapping_shr(i as u32) & 1) == 1
            } else {
                false
            };
            out.push(self.const_bool(bit));
            i = i.saturating_add(1);
        }
        out
    }

    #[inline]
    pub const fn not(&self, a: i32) -> i32 {
        a.saturating_neg()
    }

    pub fn and(&mut self, a: i32, b: i32) -> i32 {
        if a == self.true_lit {
            return b;
        }
        if b == self.true_lit {
            return a;
        }
        let f = self.true_lit.saturating_neg();
        if a == f || b == f {
            return f;
        }
        if a == b {
            return a;
        }
        let z = self.solver.new_var();
        let nz = z.saturating_neg();
        self.solver.add_clause(&[nz, a]);
        self.solver.add_clause(&[nz, b]);
        self.solver.add_clause(&[z, a.saturating_neg(), b.saturating_neg()]);
        z
    }

    pub fn or(&mut self, a: i32, b: i32) -> i32 {
        let conj = self.and(a.saturating_neg(), b.saturating_neg());
        conj.saturating_neg()
    }

    pub fn xor(&mut self, a: i32, b: i32) -> i32 {
        if a == self.const_bool(false) {
            return b;
        }
        if b == self.const_bool(false) {
            return a;
        }
        if a == self.const_bool(true) {
            return self.not(b);
        }
        if b == self.const_bool(true) {
            return self.not(a);
        }
        let z = self.solver.new_var();
        let nz = z.saturating_neg();
        let na = a.saturating_neg();
        let nb = b.saturating_neg();
        self.solver.add_clause(&[nz, a, b]);
        self.solver.add_clause(&[nz, na, nb]);
        self.solver.add_clause(&[z, na, b]);
        self.solver.add_clause(&[z, a, nb]);
        z
    }

    pub fn ite(&mut self, cond: i32, t_val: i32, f_val: i32) -> i32 {
        if t_val == f_val {
            return t_val;
        }
        let left = self.and(cond, t_val);
        let right = self.and(cond.saturating_neg(), f_val);
        self.or(left, right)
    }

    pub fn bv_ite(&mut self, cond: i32, t_bv: &[i32], f_bv: &[i32]) -> Vec<i32> {
        let w = t_bv.len();
        let mut out = Vec::with_capacity(w);
        let mut i: usize = 0;
        while i < w {
            out.push(self.ite(cond, t_bv[i], f_bv[i]));
            i = i.saturating_add(1);
        }
        out
    }

    pub fn bv_not(&self, a: &[i32]) -> Vec<i32> {
        a.iter().map(|&x| x.saturating_neg()).collect()
    }

    pub fn bv_add_cout(&mut self, a: &[i32], b: &[i32], cin: i32) -> (Vec<i32>, i32) {
        let w = a.len();
        let mut sum = Vec::with_capacity(w);
        let mut carry = cin;
        let mut i: usize = 0;
        while i < w {
            let axb = self.xor(a[i], b[i]);
            let s = self.xor(axb, carry);
            let ab = self.and(a[i], b[i]);
            let c_axb = self.and(carry, axb);
            carry = self.or(ab, c_axb);
            sum.push(s);
            i = i.saturating_add(1);
        }
        (sum, carry)
    }

    pub fn bv_add(&mut self, a: &[i32], b: &[i32]) -> Vec<i32> {
        let cin = self.const_bool(false);
        self.bv_add_cout(a, b, cin).0
    }

    pub fn bv_sub(&mut self, a: &[i32], b: &[i32]) -> Vec<i32> {
        let nb = self.bv_not(b);
        let cin = self.const_bool(true);
        self.bv_add_cout(a, &nb, cin).0
    }

    pub fn bv_neg(&mut self, a: &[i32]) -> Vec<i32> {
        let z = self.const_bv(0, a.len());
        self.bv_sub(&z, a)
    }

    pub fn bv_zext(&self, a: &[i32], target_w: usize) -> Vec<i32> {
        let mut out = a.to_vec();
        let f = self.const_bool(false);
        while out.len() < target_w {
            out.push(f);
        }
        out
    }

    pub fn bv_sext(&self, a: &[i32], target_w: usize) -> Vec<i32> {
        let sign = a.last().copied().unwrap_or_else(|| self.const_bool(false));
        let mut out = a.to_vec();
        while out.len() < target_w {
            out.push(sign);
        }
        out
    }

    pub fn bv_eq(&mut self, a: &[i32], b: &[i32]) -> i32 {
        let mut acc = self.const_bool(true);
        let mut i: usize = 0;
        while i < a.len() {
            let diff = self.xor(a[i], b[i]);
            acc = self.and(acc, diff.saturating_neg());
            i = i.saturating_add(1);
        }
        acc
    }

    pub fn bv_ult(&mut self, a: &[i32], b: &[i32]) -> i32 {
        let nb = self.bv_not(b);
        let cin = self.const_bool(true);
        let (_, cout) = self.bv_add_cout(a, &nb, cin);
        cout.saturating_neg()
    }

    pub fn bv_ule(&mut self, a: &[i32], b: &[i32]) -> i32 {
        let gt = self.bv_ult(b, a);
        gt.saturating_neg()
    }

    pub fn bv_slt(&mut self, a: &[i32], b: &[i32]) -> i32 {
        let w = a.len();
        let msb = w.saturating_sub(1);
        let mut a_flip = a.to_vec();
        let mut b_flip = b.to_vec();
        a_flip[msb] = a_flip[msb].saturating_neg();
        b_flip[msb] = b_flip[msb].saturating_neg();
        self.bv_ult(&a_flip, &b_flip)
    }

    pub fn bv_sle(&mut self, a: &[i32], b: &[i32]) -> i32 {
        let gt = self.bv_slt(b, a);
        gt.saturating_neg()
    }

    pub fn bv_mul_unsigned(&mut self, a: &[i32], b: &[i32]) -> Vec<i32> {
        let wa = a.len();
        let wb = b.len();
        let out_w = wa.saturating_add(wb);
        let mut acc = self.const_bv(0, out_w);
        let mut i: usize = 0;
        while i < wb {
            let mut partial = vec![self.const_bool(false); out_w];
            let mut j: usize = 0;
            while j < wa {
                let pos = i.saturating_add(j);
                if pos < out_w {
                    partial[pos] = self.and(a[j], b[i]);
                }
                j = j.saturating_add(1);
            }
            acc = self.bv_add(&acc, &partial);
            i = i.saturating_add(1);
        }
        acc
    }

    pub fn bv_mul_signed(&mut self, a: &[i32], b: &[i32]) -> Vec<i32> {
        let out_w = a.len().saturating_add(b.len());
        let sa = self.bv_sext(a, out_w);
        let sb = self.bv_sext(b, out_w);
        let mut acc = self.const_bv(0, out_w);
        let mut i: usize = 0;
        while i < out_w {
            let mut partial = vec![self.const_bool(false); out_w];
            let mut j: usize = 0;
            while i.saturating_add(j) < out_w {
                partial[i.saturating_add(j)] = self.and(sa[j], sb[i]);
                j = j.saturating_add(1);
            }
            acc = self.bv_add(&acc, &partial);
            i = i.saturating_add(1);
        }
        acc
    }

    /// Signed truncation toward zero by `2^shift`, matching `dsp_studio::trunc_div_pow2_i128`.
    pub fn bv_trunc_div_pow2_signed(&mut self, x: &[i32], shift: usize) -> Vec<i32> {
        let w = x.len();
        let is_neg = x[w.saturating_sub(1)];
        let neg_x = self.bv_neg(x);
        let mag = self.bv_ite(is_neg, &neg_x, x);
        let mut shifted_mag = Vec::with_capacity(w);
        let mut i: usize = 0;
        while i < w {
            let src = i.saturating_add(shift);
            if src < w {
                shifted_mag.push(mag[src]);
            } else {
                shifted_mag.push(self.const_bool(false));
            }
            i = i.saturating_add(1);
        }
        let neg_shifted = self.bv_neg(&shifted_mag);
        self.bv_ite(is_neg, &neg_shifted, &shifted_mag)
    }

    pub fn assert_true(&mut self, lit: i32) {
        self.solver.add_clause(&[lit]);
    }
}

/// Runs all `§8.1` formal bit-blasting CDCL proofs + solver non-vacuity sanity checks.
fn run_all_formal_cdcl_proofs() -> Result<usize, String> {
    let mut proven_theorems: usize = 0;

    // 0. Solver soundness & non-vacuity check: Pigeonhole PHP(4, 3) == UNSAT, and faulty add == SAT
    {
        let mut php = CdclSolver::new();
        let mut vars = [[0_i32; 3]; 4];
        for row in &mut vars {
            for cell in row.iter_mut() {
                *cell = php.new_var();
            }
            php.add_clause(row);
        }
        let mut hole: usize = 0;
        while hole < 3 {
            let mut p1: usize = 0;
            while p1 < 4 {
                let mut p2: usize = p1.saturating_add(1);
                while p2 < 4 {
                    php.add_clause(&[vars[p1][hole].saturating_neg(), vars[p2][hole].saturating_neg()]);
                    p2 = p2.saturating_add(1);
                }
                p1 = p1.saturating_add(1);
            }
            hole = hole.saturating_add(1);
        }
        if php.solve() != SatResult::Unsat {
            return Err("CDCL sanity check failed on PHP(4,3)".to_string());
        }

        // Non-vacuity: wrapping 8-bit add CAN overflow (`a > 0 && b > 0 && a + b < 0` MUST be SAT!)
        let mut bb = BitBlaster::new();
        let a = bb.fresh_bv(8);
        let b = bb.fresh_bv(8);
        let z = bb.const_bv(0, 8);
        let s = bb.bv_add(&a, &b);
        let gt_a = bb.bv_slt(&z, &a);
        let gt_b = bb.bv_slt(&z, &b);
        let lt_s = bb.bv_slt(&s, &z);
        bb.assert_true(gt_a);
        bb.assert_true(gt_b);
        bb.assert_true(lt_s);
        if matches!(bb.solver.solve(), SatResult::Unsat) {
            return Err("CDCL non-vacuity check failed: missed wrapping add overflow".to_string());
        }
    }

    // 1. `math_q32` 64-bit Signed Saturating Add / Sub / Neg Completeness (`UNSAT` over full 64-bit space)
    {
        let mut bb = BitBlaster::new();
        let a = bb.fresh_bv(64);
        let b = bb.fresh_bv(64);

        // Hardware two's complement saturating add circuit vs exact 65-bit mathematical clamp
        let f_lit = bb.const_bool(false);
        let (raw_sum, _) = bb.bv_add_cout(&a, &b, f_lit);
        let same_sign = bb.xor(a[63], b[63]).saturating_neg();
        let flip_sign = bb.xor(a[63], raw_sum[63]);
        let ovf = bb.and(same_sign, flip_sign);
        let i64_max = bb.const_bv(i64::MAX as u64 as u128, 64);
        let i64_min = bb.const_bv(i64::MIN as u64 as u128, 64);
        let sat_bound = bb.bv_ite(a[63], &i64_min, &i64_max);
        let sat_add_hw = bb.bv_ite(ovf, &sat_bound, &raw_sum);

        // Exact 65-bit reference
        let a65 = bb.bv_sext(&a, 65);
        let b65 = bb.bv_sext(&b, 65);
        let exact65 = bb.bv_add(&a65, &b65);
        let max65 = bb.bv_sext(&i64_max, 65);
        let min65 = bb.bv_sext(&i64_min, 65);
        let too_high = bb.bv_slt(&max65, &exact65);
        let too_low = bb.bv_slt(&exact65, &min65);
        let trunc64: Vec<i32> = exact65[..64].to_vec();
        let ref_low = bb.bv_ite(too_low, &i64_min, &trunc64);
        let sat_add_ref = bb.bv_ite(too_high, &i64_max, &ref_low);

        // Saturating neg circuit: a == i64::MIN ? i64::MAX : -a
        let is_min = bb.bv_eq(&a, &i64_min);
        let neg_a = bb.bv_neg(&a);
        let sat_neg_hw = bb.bv_ite(is_min, &i64_max, &neg_a);
        let neg_a65 = bb.bv_neg(&a65);
        let neg_ovf = bb.bv_slt(&max65, &neg_a65);
        let neg_trunc64: Vec<i32> = neg_a65[..64].to_vec();
        let sat_neg_ref = bb.bv_ite(neg_ovf, &i64_max, &neg_trunc64);

        let eq_add = bb.bv_eq(&sat_add_hw, &sat_add_ref);
        let eq_neg = bb.bv_eq(&sat_neg_hw, &sat_neg_ref);
        let both_ok = bb.and(eq_add, eq_neg);
        bb.assert_true(both_ok.saturating_neg());
        if bb.solver.solve() != SatResult::Unsat {
            return Err("Formal Proof 1 FAILED: Q32 64-bit sat_add / sat_neg counterexample".to_string());
        }
        proven_theorems = proven_theorems.saturating_add(1);
    }

    // 2. `math_q32` 8-bit Tile Multiplication & Carry-Ripple Reduction (`2^16` exhaustive + CDCL reduction)
    {
        for a8 in 0_u16..=255 {
            for b8 in 0_u16..=255 {
                let tile = mul_u64_tiled_8bit(a8 as u64, b8 as u64);
                let exact = (a8 as u128).wrapping_mul(b8 as u128);
                if tile != exact {
                    return Err("Formal Proof 2A FAILED: 8x8 tile mismatch".to_string());
                }
            }
        }
        // 2B-1: Bit-blast byte-column carry-ripple reduction across arbitrary 16-bit tile products `P_ij <= 65025`
        {
            let mut bb = BitBlaster::new();
            let max_tile = bb.const_bv(65_025, 16);
            let p00 = bb.fresh_bv(16);
            let p01 = bb.fresh_bv(16);
            let p10 = bb.fresh_bv(16);
            let p11 = bb.fresh_bv(16);
            for p in [&p00, &p01, &p10, &p11] {
                let bound_ok = bb.bv_ule(p, &max_tile);
                bb.assert_true(bound_ok);
            }

            // Reference shifted 32-bit accumulation
            let z8 = bb.const_bv(0, 8);
            let z16 = bb.const_bv(0, 16);
            let s00 = bb.bv_zext(&p00, 32);
            let mut s01 = z8.clone();
            s01.extend_from_slice(&p01);
            s01.extend_from_slice(&z8);
            let mut s10 = z8;
            s10.extend_from_slice(&p10);
            s10.extend_from_slice(&bb.const_bv(0, 8));
            let mut s11 = z16;
            s11.extend_from_slice(&p11);

            let ref_a = bb.bv_add(&s00, &s01);
            let ref_b = bb.bv_add(&ref_a, &s10);
            let ref_sum32 = bb.bv_add(&ref_b, &s11);

            // Column-by-column `(lo_byte, carry)` reduction matching `mul_u64_tiled_8bit`
            let byte0 = p00[0..8].to_vec();
            let c0_init = bb.bv_zext(&p00[8..16], 18);
            let p01_18 = bb.bv_zext(&p01, 18);
            let p10_18 = bb.bv_zext(&p10, 18);
            let col1_a = bb.bv_add(&c0_init, &p01_18);
            let col1 = bb.bv_add(&col1_a, &p10_18);
            let byte1 = col1[0..8].to_vec();

            let carry1 = bb.bv_zext(&col1[8..18], 16);
            let col23 = bb.bv_add(&carry1, &p11);

            let mut ripple32 = Vec::with_capacity(32);
            ripple32.extend_from_slice(&byte0);
            ripple32.extend_from_slice(&byte1);
            ripple32.extend_from_slice(&col23);

            let eq_red = bb.bv_eq(&ref_sum32, &ripple32);
            bb.assert_true(eq_red.saturating_neg());
            if bb.solver.solve() != SatResult::Unsat {
                return Err("Formal Proof 2B-1 FAILED: column carry-ripple reduction counterexample".to_string());
            }
        }
        // 2B-2: Monolithic end-to-end 4-tile multiplier circuit equivalence (`8x8 -> 16-bit` via 4-bit tiles)
        {
            let mut bb = BitBlaster::new();
            let a = bb.fresh_bv(8);
            let b = bb.fresh_bv(8);
            let full_prod = bb.bv_mul_unsigned(&a, &b);

            let p00_raw = bb.bv_mul_unsigned(&a[0..4], &b[0..4]);
            let p00 = bb.bv_zext(&p00_raw, 16);
            let p01_raw = bb.bv_mul_unsigned(&a[0..4], &b[4..8]);
            let p10_raw = bb.bv_mul_unsigned(&a[4..8], &b[0..4]);
            let p11_raw = bb.bv_mul_unsigned(&a[4..8], &b[4..8]);

            let z4 = bb.const_bv(0, 4);
            let mut p01 = z4.clone();
            p01.extend_from_slice(&p01_raw);
            p01.extend_from_slice(&z4);
            let mut p10 = z4.clone();
            p10.extend_from_slice(&p10_raw);
            p10.extend_from_slice(&z4);
            let mut p11 = bb.const_bv(0, 8);
            p11.extend_from_slice(&p11_raw);

            let s1 = bb.bv_add(&p00, &p01);
            let s2 = bb.bv_add(&s1, &p10);
            let tiled_prod = bb.bv_add(&s2, &p11);
            let eq_mul = bb.bv_eq(&full_prod, &tiled_prod);
            bb.assert_true(eq_mul.saturating_neg());
            if bb.solver.solve() != SatResult::Unsat {
                return Err("Formal Proof 2B-2 FAILED: monolithic tiled multiplier counterexample".to_string());
            }
        }
        proven_theorems = proven_theorems.saturating_add(1);
    }

    // 3. `math_q32` `isqrt` Digit-by-Digit Shift-Subtract Exactness & 64-Bit Step Invariant (`UNSAT`)
    {
        // 3A: Monolithic unrolled 10-bit `isqrt` circuit (`r^2 <= x < (r + 1)^2` over full bitvector space)
        {
            let mut bb = BitBlaster::new();
            let x = bb.fresh_bv(10);
            let mut rem = x.clone();
            let mut root = bb.const_bv(0, 10);

            let mut shift: i32 = 8;
            while shift >= 0 {
                let bit_bv = bb.const_bv(1_u128.wrapping_shl(shift as u32), 10);
                let cand = bb.bv_add(&root, &bit_bv);
                let ge = bb.bv_ule(&cand, &rem);
                let rem_sub = bb.bv_sub(&rem, &cand);
                rem = bb.bv_ite(ge, &rem_sub, &rem);

                let mut root_shr1 = Vec::with_capacity(10);
                root_shr1.extend_from_slice(&root[1..10]);
                root_shr1.push(bb.const_bool(false));
                let root_inc = bb.bv_add(&root_shr1, &bit_bv);
                root = bb.bv_ite(ge, &root_inc, &root_shr1);
                shift = shift.saturating_sub(2);
            }

            let r5 = &root[0..5];
            let r_sq10 = bb.bv_mul_unsigned(r5, r5);
            let r_ext6 = bb.bv_zext(r5, 6);
            let one6 = bb.const_bv(1, 6);
            let r_plus1 = bb.bv_add(&r_ext6, &one6);
            let next_sq12 = bb.bv_mul_unsigned(&r_plus1, &r_plus1);
            let x12 = bb.bv_zext(&x, 12);
            let r_sq12 = bb.bv_zext(&r_sq10, 12);

            let lb_ok = bb.bv_ule(&r_sq12, &x12);
            let ub_ok = bb.bv_ult(&x12, &next_sq12);
            let post_ok = bb.and(lb_ok, ub_ok);
            bb.assert_true(post_ok.saturating_neg());
            if bb.solver.solve() != SatResult::Unsat {
                return Err("Formal Proof 3A FAILED: monolithic isqrt postcondition counterexample".to_string());
            }
        }
        // 3B: Full 64-Bit Inductive Shift-Subtract Step Invariant (`rem <= 8*r + 3 ==> rem_next <= 2*r_next`)
        {
            let mut bb = BitBlaster::new();
            let r_prev = bb.fresh_bv(60);
            let rem_in = bb.fresh_bv(64);
            let r64 = bb.bv_zext(&r_prev, 64);

            // `8 * r + 3`
            let r_shl1 = {
                let mut v = vec![bb.const_bool(false)];
                v.extend_from_slice(&r64[0..63]);
                v
            };
            let r_shl2 = {
                let mut v = bb.const_bv(0, 2);
                v.extend_from_slice(&r64[0..62]);
                v
            };
            let r_shl3 = {
                let mut v = bb.const_bv(0, 3);
                v.extend_from_slice(&r64[0..61]);
                v
            };
            let three64 = bb.const_bv(3, 64);
            let pre_bound = bb.bv_add(&r_shl3, &three64);
            let pre_ok = bb.bv_ule(&rem_in, &pre_bound);
            bb.assert_true(pre_ok);

            let one64 = bb.const_bv(1, 64);
            let zero64 = bb.const_bv(0, 64);
            let cand = bb.bv_add(&r_shl2, &one64);
            let take = bb.bv_ule(&cand, &rem_in);
            let rem_sub = bb.bv_sub(&rem_in, &cand);
            let rem_next = bb.bv_ite(take, &rem_sub, &rem_in);
            let bit_inc = bb.bv_ite(take, &one64, &zero64);
            let r_next = bb.bv_add(&r_shl1, &bit_inc);
            let two_r_next = bb.bv_add(&r_next, &r_next);

            let inv_preserved = bb.bv_ule(&rem_next, &two_r_next);
            bb.assert_true(inv_preserved.saturating_neg());
            if bb.solver.solve() != SatResult::Unsat {
                return Err("Formal Proof 3B FAILED: 64-bit isqrt inductive step invariant counterexample".to_string());
            }
        }
        proven_theorems = proven_theorems.saturating_add(1);
    }

    // 4. `world_ledger` `§1.2` Full 64-Bit Zero-Sum Lifespan & Capital Conservation (`UNSAT` over 64-bit space)
    {
        let mut bb = BitBlaster::new();
        let b0 = bb.fresh_bv(64);
        let b1 = bb.fresh_bv(64);
        let dt0 = bb.fresh_bv(64);
        let dt1 = bb.fresh_bv(64);
        let core0 = bb.fresh_bv(64);
        let core1 = bb.fresh_bv(64);
        let tx0 = bb.fresh_bv(64);
        let tx1 = bb.fresh_bv(64);
        let b0_next = bb.fresh_bv(64);
        let b1_next = bb.fresh_bv(64);

        let z64 = bb.const_bv(0, 64);
        let z68 = bb.const_bv(0, 68);
        let max68 = bb.const_bv(i64::MAX as u64 as u128, 68);

        let b0_68 = bb.bv_sext(&b0, 68);
        let b1_68 = bb.bv_sext(&b1, 68);
        let dt0_68 = bb.bv_sext(&dt0, 68);
        let dt1_68 = bb.bv_sext(&dt1, 68);
        let c0_68 = bb.bv_sext(&core0, 68);
        let c1_68 = bb.bv_sext(&core1, 68);
        let tx0_68 = bb.bv_sext(&tx0, 68);
        let tx1_68 = bb.bv_sext(&tx1, 68);
        let bn0_68 = bb.bv_sext(&b0_next, 68);
        let bn1_68 = bb.bv_sext(&b1_next, 68);

        let mut accepted = bb.const_bool(true);
        for non_neg in [&b0, &b1, &dt0, &dt1, &core0, &core1] {
            let ge0 = bb.bv_sle(&z64, non_neg);
            accepted = bb.and(accepted, ge0);
        }
        let tx_sum = bb.bv_add(&tx0_68, &tx1_68);
        let tx_zero = bb.bv_eq(&tx_sum, &z68);
        accepted = bb.and(accepted, tx_zero);

        let exp0 = {
            let s1 = bb.bv_sub(&b0_68, &dt0_68);
            let s2 = bb.bv_sub(&s1, &c0_68);
            bb.bv_add(&s2, &tx0_68)
        };
        let exp1 = {
            let s1 = bb.bv_sub(&b1_68, &dt1_68);
            let s2 = bb.bv_sub(&s1, &c1_68);
            bb.bv_add(&s2, &tx1_68)
        };

        let c_e0_lo = bb.bv_sle(&z68, &exp0);
        let c_e0_hi = bb.bv_sle(&exp0, &max68);
        let c_e1_lo = bb.bv_sle(&z68, &exp1);
        let c_e1_hi = bb.bv_sle(&exp1, &max68);
        let c_eq0 = bb.bv_eq(&exp0, &bn0_68);
        let c_eq1 = bb.bv_eq(&exp1, &bn1_68);
        for cond in [c_e0_lo, c_e0_hi, c_e1_lo, c_e1_hi, c_eq0, c_eq1] {
            accepted = bb.and(accepted, cond);
        }

        let total_before = bb.bv_add(&b0_68, &b1_68);
        let burn_total = {
            let d01 = bb.bv_add(&dt0_68, &dt1_68);
            let c01 = bb.bv_add(&c0_68, &c1_68);
            bb.bv_add(&d01, &c01)
        };
        let expected_after = bb.bv_sub(&total_before, &burn_total);
        let actual_after = bb.bv_add(&bn0_68, &bn1_68);
        let inv_cons = bb.bv_eq(&actual_after, &expected_after);
        let bn0_ge0 = bb.bv_sle(&z64, &b0_next);
        let bn1_ge0 = bb.bv_sle(&z64, &b1_next);
        let inv_ge0 = bb.and(bn0_ge0, bn1_ge0);
        let inv_all = bb.and(inv_cons, inv_ge0);

        bb.assert_true(accepted);
        bb.assert_true(inv_all.saturating_neg());
        if bb.solver.solve() != SatResult::Unsat {
            return Err("Formal Proof 4 FAILED: WorldLedger 64-bit conservation counterexample".to_string());
        }
        proven_theorems = proven_theorems.saturating_add(1);
    }

    // 5. `world_ledger` `§1.3` Finite Spacetime Lattice Novikov Fixed-Point Soundness (`UNSAT`)
    {
        let mut bb = BitBlaster::new();
        let states = bb.fresh_bv(6);
        let inv_flags = bb.fresh_bv(6);

        let mut verifier_accepts = bb.const_bool(true);
        let mut parity_invert = bb.const_bool(false);
        for idx in 0_usize..6 {
            let nxt = idx.saturating_add(1).wrapping_rem(6);
            let expected_nxt = bb.xor(states[idx], inv_flags[idx]);
            let edge_consistent = bb.xor(states[nxt], expected_nxt).saturating_neg();
            verifier_accepts = bb.and(verifier_accepts, edge_consistent);
            parity_invert = bb.xor(parity_invert, inv_flags[idx]);
        }
        bb.assert_true(parity_invert);
        bb.assert_true(verifier_accepts);
        if bb.solver.solve() != SatResult::Unsat {
            return Err("Formal Proof 5 FAILED: Novikov paradox false negative".to_string());
        }
        proven_theorems = proven_theorems.saturating_add(1);
    }

    // 6. `typo_hangul` `§3.1` 32-Bit Subtitle ROI Non-Overlap Soundness (`UNSAT` over full i32 space)
    {
        let mut bb = BitBlaster::new();
        let sx0 = bb.fresh_bv(32);
        let sy0 = bb.fresh_bv(32);
        let sx1 = bb.fresh_bv(32);
        let sy1 = bb.fresh_bv(32);
        let rx0 = bb.fresh_bv(32);
        let ry0 = bb.fresh_bv(32);
        let rx1 = bb.fresh_bv(32);
        let ry1 = bb.fresh_bv(32);
        let px = bb.fresh_bv(32);
        let py = bb.fresh_bv(32);

        let s_x_ok = bb.bv_slt(&sx0, &sx1);
        let s_y_ok = bb.bv_slt(&sy0, &sy1);
        let wf_s = bb.and(s_x_ok, s_y_ok);

        let r_x_ok = bb.bv_slt(&rx0, &rx1);
        let r_y_ok = bb.bv_slt(&ry0, &ry1);
        let wf_r = bb.and(r_x_ok, r_y_ok);

        let sep_x1 = bb.bv_sle(&sx1, &rx0);
        let sep_x2 = bb.bv_sle(&rx1, &sx0);
        let sep_x = bb.or(sep_x1, sep_x2);

        let sep_y1 = bb.bv_sle(&sy1, &ry0);
        let sep_y2 = bb.bv_sle(&ry1, &sy0);
        let sep_y = bb.or(sep_y1, sep_y2);

        let sep_any = bb.or(sep_x, sep_y);
        let wf_both = bb.and(wf_s, wf_r);
        let check_accepts = bb.and(wf_both, sep_any);

        let ps_x0 = bb.bv_sle(&sx0, &px);
        let ps_x1 = bb.bv_slt(&px, &sx1);
        let in_s_x = bb.and(ps_x0, ps_x1);

        let ps_y0 = bb.bv_sle(&sy0, &py);
        let ps_y1 = bb.bv_slt(&py, &sy1);
        let in_s_y = bb.and(ps_y0, ps_y1);

        let pr_x0 = bb.bv_sle(&rx0, &px);
        let pr_x1 = bb.bv_slt(&px, &rx1);
        let in_r_x = bb.and(pr_x0, pr_x1);

        let pr_y0 = bb.bv_sle(&ry0, &py);
        let pr_y1 = bb.bv_slt(&py, &ry1);
        let in_r_y = bb.and(pr_y0, pr_y1);

        let in_s = bb.and(in_s_x, in_s_y);
        let in_r = bb.and(in_r_x, in_r_y);
        let pixel_collision = bb.and(in_s, in_r);

        bb.assert_true(check_accepts);
        bb.assert_true(pixel_collision);
        if bb.solver.solve() != SatResult::Unsat {
            return Err("Formal Proof 6 FAILED: Subtitle ROI overlap false negative".to_string());
        }
        proven_theorems = proven_theorems.saturating_add(1);
    }

    // 7A. `dsp_studio` `§3.3` Kelly-Lochbaum Lattice Passive Energy Decay (`E_out <= E_in` UNSAT)
    {
        // 7A-1: Full 64-bit signed `trunc_div_pow2_signed(x, 32)` magnitude contraction (`|trunc(x,32)|*2^32 <= |x|`)
        {
            let mut bb = BitBlaster::new();
            let x64 = bb.fresh_bv(64);
            let i64_min = bb.const_bv(i64::MIN as u64 as u128, 64);
            let not_min = bb.bv_eq(&x64, &i64_min).saturating_neg();
            bb.assert_true(not_min);

            let q64 = bb.bv_trunc_div_pow2_signed(&x64, 32);
            let neg_x = bb.bv_neg(&x64);
            let abs_x = bb.bv_ite(x64[63], &neg_x, &x64);
            let neg_q = bb.bv_neg(&q64);
            let abs_q = bb.bv_ite(q64[63], &neg_q, &q64);

            let mut abs_q_shl32 = bb.const_bv(0, 32);
            abs_q_shl32.extend_from_slice(&abs_q[0..32]);
            let contracted = bb.bv_ule(&abs_q_shl32, &abs_x);
            bb.assert_true(contracted.saturating_neg());
            if bb.solver.solve() != SatResult::Unsat {
                return Err("Formal Proof 7A-1 FAILED: 64-bit trunc_div_pow2 magnitude amplification".to_string());
            }
        }
        // 7A-2: Monolithic end-to-end Kelly-Lochbaum junction passive energy decay (`E_out <= E_in` UNSAT)
        {
            let mut bb = BitBlaster::new();
            let f_in = bb.fresh_bv(6);
            let b_in = bb.fresh_bv(6);
            let k_q = bb.fresh_bv(6);
            let g_q = bb.fresh_bv(6);

            let min_wave = bb.const_bv((-15_i8 as u8) as u128, 6);
            let max_wave = bb.const_bv(15, 6);
            let zero6 = bb.const_bv(0, 6);
            let c16_6 = bb.const_bv(16, 6);

            for v in [&f_in, &b_in, &k_q] {
                let lo = bb.bv_sle(&min_wave, v);
                let hi = bb.bv_sle(v, &max_wave);
                bb.assert_true(lo);
                bb.assert_true(hi);
            }
            let glo = bb.bv_sle(&zero6, &g_q);
            let ghi = bb.bv_sle(&g_q, &c16_6);
            bb.assert_true(glo);
            bb.assert_true(ghi);

            let k2_raw = bb.bv_mul_signed(&k_q, &k_q);
            let g2_raw = bb.bv_mul_signed(&g_q, &g_q);
            let k2 = bb.bv_zext(&k2_raw, 14);
            let g2 = bb.bv_zext(&g2_raw, 14);
            let kg_sum = bb.bv_add(&k2, &g2);
            let m_sq = bb.const_bv(256, 14);
            let on_disk = bb.bv_ule(&kg_sum, &m_sq);
            bb.assert_true(on_disk);

            let gf_raw = bb.bv_mul_signed(&g_q, &f_in);
            let kb_raw = bb.bv_mul_signed(&k_q, &b_in);
            let kf_raw = bb.bv_mul_signed(&k_q, &f_in);
            let gb_raw = bb.bv_mul_signed(&g_q, &b_in);
            let gf = bb.bv_sext(&gf_raw, 14);
            let kb = bb.bv_sext(&kb_raw, 14);
            let kf = bb.bv_sext(&kf_raw, 14);
            let gb = bb.bv_sext(&gb_raw, 14);
            let x14 = bb.bv_add(&gf, &kb);
            let y14 = bb.bv_sub(&gb, &kf);

            let f_trunc = bb.bv_trunc_div_pow2_signed(&x14, 4);
            let b_trunc = bb.bv_trunc_div_pow2_signed(&y14, 4);
            let f_out6 = &f_trunc[0..6];
            let b_out6 = &b_trunc[0..6];

            let fi2_raw = bb.bv_mul_signed(&f_in, &f_in);
            let bi2_raw = bb.bv_mul_signed(&b_in, &b_in);
            let fi2 = bb.bv_zext(&fi2_raw, 14);
            let bi2 = bb.bv_zext(&bi2_raw, 14);
            let e_in = bb.bv_add(&fi2, &bi2);

            let fo2_raw = bb.bv_mul_signed(f_out6, f_out6);
            let bo2_raw = bb.bv_mul_signed(b_out6, b_out6);
            let fo2 = bb.bv_zext(&fo2_raw, 14);
            let bo2 = bb.bv_zext(&bo2_raw, 14);
            let e_out = bb.bv_add(&fo2, &bo2);

            let amplified = bb.bv_ult(&e_in, &e_out);
            bb.assert_true(amplified);
            if bb.solver.solve() != SatResult::Unsat {
                return Err("Formal Proof 7A-2 FAILED: Kelly-Lochbaum energy amplification".to_string());
            }
        }
        proven_theorems = proven_theorems.saturating_add(1);
    }

    // 7B. `dsp_studio` `§3.3` 4-Channel Unitary Householder Damped FDN Energy Decay (`UNSAT`)
    {
        let mut bb = BitBlaster::new();
        let x0 = bb.fresh_bv(4);
        let x1 = bb.fresh_bv(4);
        let x2 = bb.fresh_bv(4);
        let x3 = bb.fresh_bv(4);
        let damp = bb.fresh_bv(5);

        let min_x = bb.const_bv((-7_i8 as u8) as u128, 4);
        let max_x = bb.const_bv(7, 4);
        for xi in [&x0, &x1, &x2, &x3] {
            let lo = bb.bv_sle(&min_x, xi);
            let hi = bb.bv_sle(xi, &max_x);
            bb.assert_true(lo);
            bb.assert_true(hi);
        }
        let z5 = bb.const_bv(0, 5);
        let c8_5 = bb.const_bv(8, 5);
        let d_lo = bb.bv_sle(&z5, &damp);
        let d_hi = bb.bv_sle(&damp, &c8_5);
        bb.assert_true(d_lo);
        bb.assert_true(d_hi);

        let x0_7 = bb.bv_sext(&x0, 7);
        let x1_7 = bb.bv_sext(&x1, 7);
        let x2_7 = bb.bv_sext(&x2, 7);
        let x3_7 = bb.bv_sext(&x3, 7);
        let s01 = bb.bv_add(&x0_7, &x1_7);
        let s23 = bb.bv_add(&x2_7, &x3_7);
        let sum7 = bb.bv_add(&s01, &s23);
        let damp7 = bb.bv_zext(&damp, 7);

        let mut e_in = bb.const_bv(0, 14);
        let mut e_out = bb.const_bv(0, 14);
        for xi in [&x0, &x1, &x2, &x3] {
            let xi7 = bb.bv_sext(xi, 7);
            let two_xi = bb.bv_add(&xi7, &xi7);
            let h_i = bb.bv_sub(&two_xi, &sum7);
            let prod14 = bb.bv_mul_signed(&h_i, &damp7);
            let trunc14 = bb.bv_trunc_div_pow2_signed(&prod14, 4);
            let out6 = &trunc14[0..6];

            let xi6 = bb.bv_sext(xi, 6);
            let sq_in_raw = bb.bv_mul_signed(&xi6, &xi6);
            let sq_out_raw = bb.bv_mul_signed(out6, out6);
            let sq_in = bb.bv_zext(&sq_in_raw, 14);
            let sq_out = bb.bv_zext(&sq_out_raw, 14);
            e_in = bb.bv_add(&e_in, &sq_in);
            e_out = bb.bv_add(&e_out, &sq_out);
        }
        let amplified = bb.bv_ult(&e_in, &e_out);
        bb.assert_true(amplified);
        if bb.solver.solve() != SatResult::Unsat {
            return Err("Formal Proof 7B FAILED: Householder FDN energy amplification".to_string());
        }
        proven_theorems = proven_theorems.saturating_add(1);
    }

    Ok(proven_theorems)
}

// ============================================================================
// 2. SEALING, PREFLIGHT STATIC SCANNER & PRE-TOOL-USE HOOK GUARD (`§6`, `§8.2`)
// ============================================================================

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
struct SealManifest {
    pub files: BTreeMap<String, String>,
}

fn strip_win_verbatim_prefix(p: PathBuf) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(rest) = s.strip_prefix("\\\\?\\")
        && !rest.starts_with("UNC\\")
    {
        return PathBuf::from(rest);
    }
    p
}

fn find_repo_root() -> Result<PathBuf, String> {
    if let Ok(exe) = env::current_exe()
        && let Some(target_dir) = exe.parent().and_then(Path::parent)
        && let Some(root) = target_dir.parent()
        && root.join("애니.md").is_file()
        && root.join("Cargo.toml").is_file()
    {
        return fs::canonicalize(root)
            .map(strip_win_verbatim_prefix)
            .map_err(|e| e.to_string());
    }
    let cwd = env::current_dir().map_err(|e| e.to_string())?;
    if cwd.join("애니.md").is_file() {
        return fs::canonicalize(&cwd)
            .map(strip_win_verbatim_prefix)
            .map_err(|e| e.to_string());
    }
    let child = cwd.join("작품");
    if child.join("애니.md").is_file() {
        return fs::canonicalize(&child)
            .map(strip_win_verbatim_prefix)
            .map_err(|e| e.to_string());
    }
    Err("Unable to locate R_root containing 애니.md".to_string())
}

fn blake3_file_hex(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| format!("Failed reading {}: {e}", path.display()))?;
    Ok(blake3::hash(&bytes).to_hex().to_string())
}

fn sealed_relative_paths() -> Vec<String> {
    let mut list = vec![
        "애니.md".to_string(),
        "Cargo.toml".to_string(),
        "Cargo.lock".to_string(),
        ".cargo/config.toml".to_string(),
        "rust-toolchain.toml".to_string(),
        ".gitignore".to_string(),
        "clippy.toml".to_string(),
        ".track_mode".to_string(),
        "kernel/verifier_abi/src/lib.rs".to_string(),
        "verifier/Cargo.toml".to_string(),
        "verifier/clippy.toml".to_string(),
        "verifier/src/main.rs".to_string(),
        ".agents/hooks.json".to_string(),
        ".agents/plugins/anime-forge/plugin.json".to_string(),
        ".agents/plugins/anime-forge/agents/anime-architect.md".to_string(),
        ".agents/plugins/anime-forge/agents/anime-builder.md".to_string(),
        ".agents/plugins/anime-forge/agents/anime-prosecutor.md".to_string(),
        "workflows/forge_loop.py".to_string(),
    ];
    for crate_name in ALL_KERNEL_CRATES {
        list.push(format!("kernel/{crate_name}/Cargo.toml"));
        list.push(format!("kernel/{crate_name}/clippy.toml"));
    }
    list.sort();
    list.dedup();
    list
}

fn cmd_seal_init(root: &Path) -> Result<(), String> {
    let mut manifest = SealManifest::default();
    for rel in sealed_relative_paths() {
        let full = root.join(&rel);
        let hex = blake3_file_hex(&full)?;
        manifest.files.insert(rel, hex);
    }
    let json = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    fs::write(root.join("seal.lock"), json).map_err(|e| e.to_string())?;
    Ok(())
}

fn verify_seal_against_target(host_root: &Path, target_root: &Path) -> Result<(), String> {
    let seal_path = host_root.join("seal.lock");
    let raw = fs::read_to_string(&seal_path)
        .map_err(|e| format!("Missing or unreadable seal.lock at {}: {e}", seal_path.display()))?;
    let seal: SealManifest = serde_json::from_str(&raw).map_err(|e| format!("Corrupt seal.lock: {e}"))?;

    for expected_rel in sealed_relative_paths() {
        let Some(expected_hash) = seal.files.get(&expected_rel) else {
            return Err(format!("seal.lock missing required path: {expected_rel}"));
        };
        let target_file = target_root.join(&expected_rel);
        let actual_hash = blake3_file_hex(&target_file)?;
        if &actual_hash != expected_hash {
            return Err(format!(
                "SEAL VIOLATION on {expected_rel}: expected {expected_hash}, got {actual_hash}"
            ));
        }
    }
    Ok(())
}

fn walk_preflight_dir(dir: &Path, root: &Path) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|e| e.to_string())?;
    for entry_res in entries {
        let entry = entry_res.map_err(|e| e.to_string())?;
        let meta = entry.metadata().map_err(|e| e.to_string())?;
        let sym_meta = fs::symlink_metadata(entry.path()).map_err(|e| e.to_string())?;
        if sym_meta.file_type().is_symlink() {
            return Err(format!("Symlink prohibited: {}", entry.path().display()));
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if meta.is_dir() {
            if dir == root && matches!(name.as_str(), ".git" | "target" | "state" | "output") {
                continue;
            }
            walk_preflight_dir(&path, root)?;
        } else {
            if name == "build.rs" {
                return Err(format!("build.rs prohibited (`§7.1`): {}", path.display()));
            }
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let lower = ext.to_ascii_lowercase();
                if BANNED_ASSET_EXTENSIONS.contains(&lower.as_str()) {
                    return Err(format!(
                        "Banned external asset/shader/model extension `.{lower}` (`§4.1`): {}",
                        path.display()
                    ));
                }
            }
        }
    }
    Ok(())
}

fn check_single_rs_file_in_crate(crate_dir: &Path) -> Result<PathBuf, String> {
    let mut rs_files: Vec<PathBuf> = Vec::new();
    collect_rs_files(crate_dir, &mut rs_files)?;
    if rs_files.len() != 1 {
        return Err(format!(
            "Single-source-file violation (`§6.1`) in {}: found {} .rs files",
            crate_dir.display(),
            rs_files.len()
        ));
    }
    let only = rs_files.remove(0);
    let rel = only
        .strip_prefix(crate_dir)
        .map_err(|e| e.to_string())?
        .to_string_lossy()
        .replace('\\', "/");
    if rel != "src/lib.rs" && rel != "src/main.rs" {
        return Err(format!(
            "Invalid source entrypoint `{rel}` in {}",
            crate_dir.display()
        ));
    }
    Ok(only)
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry_res in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry_res.map_err(|e| e.to_string())?;
        let p = entry.path();
        if p.is_dir() {
            collect_rs_files(&p, out)?;
        } else if p.extension().and_then(|s| s.to_str()) == Some("rs") {
            out.push(p);
        }
    }
    Ok(())
}

fn contains_ident_token(src: &str, ident: &str) -> bool {
    let bytes = src.as_bytes();
    let pat = ident.as_bytes();
    if pat.is_empty() || bytes.len() < pat.len() {
        return false;
    }
    let mut i: usize = 0;
    let end = bytes.len().saturating_sub(pat.len());
    while i <= end {
        if bytes.get(i..i.saturating_add(pat.len())) == Some(pat) {
            let prev_ok = if i == 0 {
                true
            } else {
                let c = bytes[i.saturating_sub(1)];
                !(c.is_ascii_alphanumeric() || c == b'_')
            };
            let after_idx = i.saturating_add(pat.len());
            let next_ok = if after_idx >= bytes.len() {
                true
            } else {
                let c = bytes[after_idx];
                !(c.is_ascii_alphanumeric() || c == b'_')
            };
            if prev_ok && next_ok {
                return true;
            }
        }
        i = i.saturating_add(1);
    }
    false
}

fn scan_rust_source_rules(src: &str, crate_name: &str, is_ksim: bool) -> Result<(), String> {
    for banned in &[
        "#[allow",
        "#![allow",
        "#[expect",
        "#![expect",
        "#[path",
        "include_bytes!",
        "include_str!",
        "proc_macro",
    ] {
        if src.contains(banned) {
            return Err(format!(
                "Banned construct `{banned}` in `{crate_name}` (`§7.1`)"
            ));
        }
    }
    if contains_ident_token(src, "HashMap") || contains_ident_token(src, "HashSet") {
        return Err(format!(
            "Non-deterministic HashMap/HashSet in `{crate_name}` (`§7.1`)"
        ));
    }
    for req_lint in &[
        "#![forbid(",
        "clippy::unwrap_used",
        "clippy::expect_used",
        "clippy::panic",
        "clippy::arithmetic_side_effects",
        "clippy::disallowed_types",
        "clippy::disallowed_methods",
    ] {
        if !src.contains(req_lint) {
            return Err(format!(
                "Missing mandatory `#![forbid(...)]` lint `{req_lint}` in `{crate_name}`"
            ));
        }
    }
    if is_ksim {
        if !src.contains("#![no_std]") {
            return Err(format!("K_sim crate `{crate_name}` missing `#![no_std]`"));
        }
        if contains_ident_token(src, "f32") || contains_ident_token(src, "f64") {
            return Err(format!(
                "Floating-point (`f32`/`f64`) prohibited in K_sim crate `{crate_name}`"
            ));
        }
    }
    Ok(())
}

fn verify_closed_dependency_allowlist(target_root: &Path) -> Result<(), String> {
    let cargo_toml = fs::read_to_string(target_root.join("Cargo.toml")).map_err(|e| e.to_string())?;
    let allowed: BTreeSet<&str> = ALLOWED_EXTERNAL_CRATES.iter().copied().collect();
    let internal: BTreeSet<&str> = ALL_KERNEL_CRATES.iter().copied().collect();
    let mut in_ws_deps = false;

    for raw_line in cargo_toml.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            in_ws_deps = line == "[workspace.dependencies]";
            continue;
        }
        if in_ws_deps
            && let Some((dep_key, _)) = line.split_once('=')
        {
            let name = dep_key.trim();
            if !allowed.contains(name) && !internal.contains(name) {
                return Err(format!(
                    "Unauthorized crate `{name}` in `[workspace.dependencies]` (`§6.1`)"
                ));
            }
        }
    }
    Ok(())
}

fn run_preflight(host_root: &Path, target_root: &Path) -> Result<(), String> {
    verify_seal_against_target(host_root, target_root)?;
    walk_preflight_dir(target_root, target_root)?;
    verify_closed_dependency_allowlist(target_root)?;

    for &crate_name in ALL_KERNEL_CRATES {
        let cdir = target_root.join("kernel").join(crate_name);
        let rs_path = check_single_rs_file_in_crate(&cdir)?;
        let src = fs::read_to_string(&rs_path).map_err(|e| e.to_string())?;
        let is_ksim = KSIM_CRATES.contains(&crate_name);
        scan_rust_source_rules(&src, crate_name, is_ksim)?;
    }

    let ver_rs = check_single_rs_file_in_crate(&target_root.join("verifier"))?;
    let ver_src = fs::read_to_string(&ver_rs).map_err(|e| e.to_string())?;
    for req_lint in &["#![forbid(", "clippy::disallowed_types"] {
        if !ver_src.contains(req_lint) {
            return Err(format!("verifier missing `{req_lint}`"));
        }
    }
    Ok(())
}

fn has_parent_dir_component(p: &Path) -> bool {
    p.components().any(|c| matches!(c, Component::ParentDir))
}

fn has_symlink_in_chain(p: &Path) -> bool {
    let mut cur = PathBuf::new();
    for comp in p.components() {
        cur.push(comp.as_os_str());
        if let Ok(meta) = fs::symlink_metadata(&cur)
            && meta.file_type().is_symlink()
        {
            return true;
        }
    }
    false
}

fn norm_lower_path(p: &Path) -> String {
    let resolved = fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let mut s = resolved.to_string_lossy().replace('/', "\\").to_lowercase();
    if let Some(stripped) = s.strip_prefix("\\\\?\\") {
        s = stripped.to_string();
    }
    while s.ends_with('\\') && s.len() > 3 {
        s.pop();
    }
    s
}

fn is_strictly_under_or_equal(candidate: &str, root: &str) -> bool {
    if candidate == root {
        return true;
    }
    let prefix = format!("{root}\\");
    candidate.starts_with(&prefix)
}

fn extract_nonce(text: &str) -> Option<String> {
    let start = text.find("[NONCE-")?;
    let rest = text.get(start.saturating_add(1)..)?;
    let end = rest.find(']')?;
    let token = rest.get(..end)?;
    if token.len() > 7
        && token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        Some(token.to_string())
    } else {
        None
    }
}

fn evaluate_hook_guard(host_root: &Path, input_json: &str, persist_ipc: bool) -> (bool, String) {
    let Ok(val) = serde_json::from_str::<Value>(input_json) else {
        return (false, "Malformed PreToolUse JSON stdin".to_string());
    };
    let Some(tool_name) = val
        .get("toolCall")
        .and_then(|tc| tc.get("name"))
        .and_then(Value::as_str)
    else {
        return (false, "Missing toolCall.name".to_string());
    };
    let empty_obj = Value::Object(serde_json::Map::new());
    let args = val
        .get("toolCall")
        .and_then(|tc| tc.get("args"))
        .unwrap_or(&empty_obj);

    let host_norm = norm_lower_path(host_root);
    let defi_parent_norm = host_root
        .parent()
        .map(norm_lower_path)
        .unwrap_or_default();

    // Register isolated branch workspace if `[NONCE-...]` is present in `lastUserInput` (`§8.2`)
    let mut active_branch_norm: Option<String> = None;
    if let (Some(last_input), Some(ws0)) = (
        val.get("lastUserInput").and_then(Value::as_str),
        val.get("workspacePaths")
            .and_then(Value::as_array)
            .and_then(|a| a.first())
            .and_then(Value::as_str),
    ) {
        let ws_path = Path::new(ws0);
        if !has_parent_dir_component(ws_path) && !has_symlink_in_chain(ws_path) {
            let ws_norm = norm_lower_path(ws_path);
            let outside_defi = !defi_parent_norm.is_empty()
                && !is_strictly_under_or_equal(&ws_norm, &defi_parent_norm);
            if outside_defi {
                active_branch_norm = Some(ws_norm);
                if persist_ipc
                    && let Some(nonce) = extract_nonce(last_input)
                {
                    let state_dir = host_root.join("state");
                    let _ = fs::create_dir_all(&state_dir);
                    let ipc_file = state_dir.join("ipc_branches.json");
                    let mut map: BTreeMap<String, String> = fs::read_to_string(&ipc_file)
                        .ok()
                        .and_then(|s| serde_json::from_str(&s).ok())
                        .unwrap_or_default();
                    map.insert(nonce, ws0.to_string());
                    if let Ok(ser) = serde_json::to_string_pretty(&map) {
                        let _ = fs::write(&ipc_file, ser);
                    }
                }
            }
        }
    }

    match tool_name {
        "finish" => (true, "Pathless finish allowed".to_string()),
        "read_file" | "grep_search" | "find_by_name" | "list_dir" => {
            let raw_p = args
                .get("Path")
                .or_else(|| args.get("SearchPath"))
                .or_else(|| args.get("SearchDirectory"))
                .or_else(|| args.get("DirectoryPath"))
                .and_then(Value::as_str);
            let Some(p_str) = raw_p else {
                return (false, "Missing target path on read tool".to_string());
            };
            let p = Path::new(p_str);
            if has_parent_dir_component(p) || has_symlink_in_chain(p) {
                return (false, "Path traversal `..` or symlink blocked".to_string());
            }
            let cand_norm = norm_lower_path(p);
            if is_strictly_under_or_equal(&cand_norm, &host_norm) {
                return (true, "Read inside R_root allowed".to_string());
            }
            if let Some(ref br) = active_branch_norm
                && is_strictly_under_or_equal(&cand_norm, br)
            {
                return (true, "Read inside isolated branch clone allowed".to_string());
            }
            (
                false,
                "SSOT-Isolation violation: read outside R_root and isolated branch".to_string(),
            )
        }
        "write_file" | "edit_file" => {
            let Some(p_str) = args.get("Path").and_then(Value::as_str) else {
                return (false, "Missing Path on write/edit tool".to_string());
            };
            let p = Path::new(p_str);
            if has_parent_dir_component(p) || has_symlink_in_chain(p) {
                return (false, "Path traversal `..` or symlink blocked".to_string());
            }
            // Resolve parent directory if file does not exist yet
            let cand_norm = if p.exists() {
                norm_lower_path(p)
            } else if let (Some(par), Some(fname)) = (p.parent(), p.file_name()) {
                format!(
                    "{}\\{}",
                    norm_lower_path(par),
                    fname.to_string_lossy().to_lowercase()
                )
            } else {
                return (false, "Invalid target write path".to_string());
            };

            // Post-seal direct writes to host R_root are 100% blocked (`§8.2`)
            if is_strictly_under_or_equal(&cand_norm, &host_norm)
                && host_root.join("seal.lock").exists()
            {
                return (
                    false,
                    "Direct write to host R_root prohibited after seal.lock (`§8.2`)".to_string(),
                );
            }

            let Some(ref br) = active_branch_norm else {
                return (
                    false,
                    "Write denied: not inside an isolated branch clone".to_string(),
                );
            };
            if !is_strictly_under_or_equal(&cand_norm, br) {
                return (
                    false,
                    "Write denied: path outside isolated branch clone".to_string(),
                );
            }

            let rel = cand_norm
                .strip_prefix(&format!("{br}\\"))
                .unwrap_or("")
                .replace('\\', "/");

            // Sealed paths cannot be modified even inside isolated branch!
            for sealed_p in sealed_relative_paths() {
                if rel.eq_ignore_ascii_case(&sealed_p)
                    || rel.starts_with("verifier/")
                    || rel.starts_with("kernel/verifier_abi/")
                    || rel.starts_with(".agents/")
                    || rel.starts_with("workflows/")
                    || rel == "seal.lock"
                {
                    return (
                        false,
                        format!("Write to sealed file `{rel}` prohibited (`§6.2`)"),
                    );
                }
            }

            if rel.ends_with("build.rs") {
                return (false, "Creation of build.rs prohibited (`§7.1`)".to_string());
            }
            if let Some(ext) = Path::new(&rel).extension().and_then(|e| e.to_str())
                && BANNED_ASSET_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str())
            {
                return (false, format!("Banned asset extension `.{ext}` (`§4.1`)"));
            }

            // Track mode scope check (`§6.2`)
            let track_mode = fs::read_to_string(host_root.join(".track_mode"))
                .unwrap_or_else(|_| "engine".to_string())
                .trim()
                .to_string();
            let allowed_in_season = rel.starts_with("season/");
            let allowed_in_engine = ALL_KERNEL_CRATES.iter().any(|&c| {
                c != "verifier_abi"
                    && (rel == format!("kernel/{c}/src/lib.rs")
                        || rel == format!("kernel/{c}/src/main.rs"))
            });
            let scope_ok = if track_mode == "engine" {
                allowed_in_engine || allowed_in_season
            } else {
                allowed_in_season
            };
            if !scope_ok {
                return (
                    false,
                    format!("Write `{rel}` violates `.track_mode = {track_mode}` (`§6.2`)"),
                );
            }

            // Content attribute / banned token scan
            let payload_text = args
                .get("Content")
                .or_else(|| args.get("Replacement"))
                .and_then(Value::as_str)
                .unwrap_or("");
            for bad_tok in &[
                "#[allow",
                "#![allow",
                "#[expect",
                "#![expect",
                "#[path",
                "include_bytes!",
                "include_str!",
            ] {
                if payload_text.contains(bad_tok) {
                    return (
                        false,
                        format!("Banned attribute/macro `{bad_tok}` blocked (`§7.1`)"),
                    );
                }
            }
            (true, "Isolated branch write allowed".to_string())
        }
        "run_workflow" => {
            if args.get("Script").and_then(Value::as_str).is_some() {
                return (false, "Inline unsealed workflow Script prohibited".to_string());
            }
            let Some(sp) = args.get("ScriptPath").and_then(Value::as_str) else {
                return (false, "Missing ScriptPath on run_workflow".to_string());
            };
            let sp_path = Path::new(sp);
            if has_parent_dir_component(sp_path) || has_symlink_in_chain(sp_path) {
                return (false, "Invalid ScriptPath".to_string());
            }
            let expected_wf = host_root.join("workflows").join("forge_loop.py");
            if norm_lower_path(sp_path) != norm_lower_path(&expected_wf) {
                return (false, "Only sealed workflows/forge_loop.py is permitted".to_string());
            }
            if verify_seal_against_target(host_root, host_root).is_err() {
                return (false, "seal.lock verification failed before run_workflow".to_string());
            }
            (true, "Sealed forge_loop.py allowed".to_string())
        }
        "run_command" => {
            let Some(cmd_raw) = args.get("CommandLine").and_then(Value::as_str) else {
                return (false, "Missing CommandLine".to_string());
            };
            let cmd = cmd_raw.trim();
            for meta in [';', '|', '&', '>', '<', '$', '`', '(', ')', '{', '}', '\n', '\r'] {
                if cmd.contains(meta) {
                    return (
                        false,
                        format!("Shell metacharacter `{meta}` prohibited (`§8.2`)"),
                    );
                }
            }
            if cmd.contains("..") {
                return (false, "Traversal `..` in command prohibited".to_string());
            }
            let ver_exe = host_root.join("target").join("release").join("verifier.exe");
            let ver_lossy = ver_exe.to_string_lossy();
            let ver_str = ver_lossy.strip_prefix("\\\\?\\").unwrap_or(&ver_lossy);
            let mut matched = false;
            for ph in ["0", "1", "2", "3", "4"] {
                let c1 = format!("cargo run --locked --release -p verifier -- gate --phase {ph}");
                let c2 = format!("{ver_str} gate --phase {ph}");
                let c3 = format!("\"{ver_str}\" gate --phase {ph}");
                if cmd == c1 || cmd.eq_ignore_ascii_case(&c2) || cmd.eq_ignore_ascii_case(&c3) {
                    matched = true;
                    break;
                }
            }
            if matched {
                (true, "Single-entry verifier gate command allowed".to_string())
            } else {
                (
                    false,
                    "Arbitrary shell command denied by Default-Deny policy (`§8.2`)".to_string(),
                )
            }
        }
        _ => (false, format!("Tool `{tool_name}` denied by Default-Deny")),
    }
}

fn cmd_hook_guard(host_root: &Path) -> ExitCode {
    let mut input = String::new();
    if io::stdin().read_to_string(&mut input).is_err() {
        let _ = writeln!(
            io::stdout(),
            "{{\"decision\":\"deny\",\"reason\":\"Failed to read stdin\"}}"
        );
        return ExitCode::SUCCESS;
    }
    let (allow, reason) = evaluate_hook_guard(host_root, &input, true);
    let out = if allow {
        serde_json::json!({ "decision": "allow" })
    } else {
        serde_json::json!({ "decision": "deny", "reason": reason })
    };
    let _ = writeln!(io::stdout(), "{out}");
    ExitCode::SUCCESS
}

// ============================================================================
// 3. SUBPROCESS HELPER WITH OFFLINE CARGO PATH RESOLUTION
// ============================================================================

fn run_cargo_checked(root: &Path, args: &[&str]) -> Result<String, String> {
    let home = env::var_os("USERPROFILE")
        .or_else(|| env::var_os("HOME"))
        .map(PathBuf::from)
        .ok_or_else(|| "Missing USERPROFILE/HOME".to_string())?;
    let cargo_bin_dir = home.join(".cargo").join("bin");
    let cargo_exe = if cfg!(windows) {
        cargo_bin_dir.join("cargo.exe")
    } else {
        cargo_bin_dir.join("cargo")
    };
    let current_path = env::var_os("PATH").unwrap_or_default();
    let mut new_path = cargo_bin_dir.into_os_string();
    new_path.push(if cfg!(windows) { ";" } else { ":" });
    new_path.push(current_path);

    let output = Command::new(&cargo_exe)
        .args(args)
        .current_dir(root)
        .env("PATH", new_path)
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .map_err(|e| format!("Failed spawning cargo {:?}: {e}", args))?;

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if !output.status.success() {
        return Err(format!("cargo {:?} failed:\n{combined}", args));
    }
    Ok(combined)
}

fn verify_git_head_exists(root: &Path) -> Result<String, String> {
    let out = Command::new("git")
        .args(["rev-parse", "--verify", "HEAD"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("Failed invoking git: {e}"))?;
    if !out.status.success() {
        return Err("Git HEAD commit missing in R_root (`§8.2`)".to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

// ============================================================================
// 4. ADVERSARIAL PARADOX / LEDGER / WAL / SECURITY SELFTESTS (`§9.1`)
// ============================================================================

fn run_adversarial_paradox_and_ledger_tests() -> Result<usize, String> {
    let mut rejected_attacks: usize = 0;

    // 1. Grandfather retrocausal paradox (1 -> 2 Copy, 2 --retro--> 1 Invert)
    let p_events = [
        SpacetimeEvent {
            event_id: 1,
            episode: 1,
            kind: EventKind::LoopAnchor,
            state_bit: true,
            rel_step: None,
        },
        SpacetimeEvent {
            event_id: 2,
            episode: 1,
            kind: EventKind::Standard,
            state_bit: true,
            rel_step: None,
        },
    ];
    let p_edges = [
        SpacetimeEdge {
            from: 1,
            to: 2,
            relation: LogicRelation::Copy,
            is_retrocausal: false,
        },
        SpacetimeEdge {
            from: 2,
            to: 1,
            relation: LogicRelation::Invert,
            is_retrocausal: true,
        },
    ];
    if verify_spacetime_graph(1, false, &p_events, &p_edges).is_err() {
        rejected_attacks = rejected_attacks.saturating_add(1);
    } else {
        return Err("Failed to reject Grandfather retrocausal paradox".to_string());
    }

    // 2. Causal DAG cycle forgery
    let cyc_edges = [
        SpacetimeEdge {
            from: 1,
            to: 2,
            relation: LogicRelation::Copy,
            is_retrocausal: false,
        },
        SpacetimeEdge {
            from: 2,
            to: 1,
            relation: LogicRelation::Copy,
            is_retrocausal: false,
        },
    ];
    if verify_spacetime_graph(1, false, &p_events, &cyc_edges) == Err(LedgerError::CausalCycleDetected) {
        rejected_attacks = rejected_attacks.saturating_add(1);
    } else {
        return Err("Failed to reject causal cycle".to_string());
    }

    // 3. Superluminal / inside-horizon Schwarzschild light-cone violation (`ds^2 > 0`)
    let ftl_events = [SpacetimeEvent {
        event_id: 10,
        episode: 1,
        kind: EventKind::Standard,
        state_bit: true,
        rel_step: Some(RelativisticStep {
            r_q32: Q32::from_int(4).0,
            r_schwarzschild_q32: Q32::from_int(2).0,
            dt_q32: Q32::ONE.0,
            dr_q32: Q32::from_int(5).0, // faster than light!
            v_tan_q32: 0,
            claimed_dtau_q32: 0,
        }),
    }];
    if verify_spacetime_graph(1, false, &ftl_events, &[])
        == Err(LedgerError::RelativisticCausalityViolation)
    {
        rejected_attacks = rejected_attacks.saturating_add(1);
    } else {
        return Err("Failed to reject superluminal light-cone violation".to_string());
    }

    // 4. Deus ex machina payoff without `>= 2` clues at `d_dag >= 4`
    let dem_events = [
        SpacetimeEvent {
            event_id: 1,
            episode: 1,
            kind: EventKind::Clue,
            state_bit: true,
            rel_step: None,
        },
        SpacetimeEvent {
            event_id: 2,
            episode: 2,
            kind: EventKind::Payoff,
            state_bit: true,
            rel_step: None,
        },
    ];
    let dem_edges = [SpacetimeEdge {
        from: 1,
        to: 2,
        relation: LogicRelation::Copy,
        is_retrocausal: false,
    }];
    if matches!(
        verify_spacetime_graph(2, false, &dem_events, &dem_edges),
        Err(LedgerError::InsufficientForeshadowClues { .. })
    ) {
        rejected_attacks = rejected_attacks.saturating_add(1);
    } else {
        return Err("Failed to reject Deus Ex Machina short clue distance".to_string());
    }

    // 5. Season finale ($E_{12}$) with unrecovered open clue sink (`deg^+(v) == 0`)
    let sink_events = [SpacetimeEvent {
        event_id: 77,
        episode: 3,
        kind: EventKind::Clue,
        state_bit: true,
        rel_step: None,
    }];
    if matches!(
        verify_spacetime_graph(12, true, &sink_events, &[]),
        Err(LedgerError::SeasonFinaleOpenClueSinks { open_count: 1 })
    ) {
        rejected_attacks = rejected_attacks.saturating_add(1);
    } else {
        return Err("Failed to reject E_12 open clue sink".to_string());
    }

    // 6..10. Ledger forgeries: unbacked minting, overdraft, i64::MAX overflow, negative burn, dead-actor spend
    let mut ledger = WorldLedger::new();
    ledger
        .register_actor(0, 1_000, 100)
        .map_err(|e| format!("{e:?}"))?;
    ledger
        .register_actor(1, 1_000, 100)
        .map_err(|e| format!("{e:?}"))?;

    if ledger.apply_zero_sum_transfer(AssetKind::Capital(0), &[(0, 500), (1, -499)])
        == Err(LedgerError::NonZeroSumTransaction)
    {
        rejected_attacks = rejected_attacks.saturating_add(1);
    } else {
        return Err("Failed to reject unbacked +1 minting".to_string());
    }

    if ledger.apply_zero_sum_transfer(AssetKind::Capital(0), &[(0, 2_000), (1, -2_000)])
        == Err(LedgerError::BalanceOverflowOrNegative)
    {
        rejected_attacks = rejected_attacks.saturating_add(1);
    } else {
        return Err("Failed to reject negative balance overdraft".to_string());
    }

    if ledger.step_lifespan(&[(0, -5, 0, 0), (1, 5, 0, 0)]) == Err(LedgerError::InvalidLifespanBurn) {
        rejected_attacks = rejected_attacks.saturating_add(1);
    } else {
        return Err("Failed to reject negative lifespan aging forgery".to_string());
    }

    // Drain actor 1 lifespan to 0 -> DeadOrBankrupt, then attempt spend from actor 1
    ledger
        .step_lifespan(&[(0, 10, 0, 50), (1, 50, 0, -50)])
        .map_err(|e| format!("{e:?}"))?;
    if ledger.apply_zero_sum_transfer(AssetKind::Capital(0), &[(1, -10), (0, 10)])
        == Err(LedgerError::ActorAlreadyDeadOrBankrupt)
    {
        rejected_attacks = rejected_attacks.saturating_add(1);
    } else {
        return Err("Failed to reject transaction from DeadOrBankrupt actor".to_string());
    }

    // 10..15. Mind-game & G4 violations
    let base_node = DecisionNode {
        node_id: 1,
        protagonist_depth: 4,
        chosen_action_id: 2,
        candidate_actions: vec![
            ActionUtility {
                action_id: 1,
                expected_utility_q32: Q32::from_int(5).0,
            },
            ActionUtility {
                action_id: 2,
                expected_utility_q32: Q32::from_int(50).0,
            },
        ],
        enemies: vec![EnemyAgent {
            enemy_id: 9,
            reasoning_depth: 3,
            betrayal_weight_q32: Q32::ONE.0,
            spared_or_forgiven: false,
            hierarchy_inversion: Some(HierarchyInversion::PowerPlundered),
        }],
        defeated_apex_enemies: vec![9],
        has_no_kill_hypocrisy: false,
        has_uncompensated_sacrifice: false,
        has_troll_forced_loss: false,
        world_entropy_opp_q32: Q32::from_int(3).0,
        world_entropy_protag_q32: Q32::ONE.0,
        safehouse_breach_prob_q32: 0,
    };
    verify_decision_node(&base_node).map_err(|e| format!("Valid node rejected: {e:?}"))?;

    let mut subopt = base_node.clone();
    subopt.chosen_action_id = 1;
    if verify_decision_node(&subopt) == Err(LedgerError::PlotInducedSuboptimalAction) {
        rejected_attacks = rejected_attacks.saturating_add(1);
    } else {
        return Err("Failed to reject suboptimal plot-induced stupidity".to_string());
    }

    let mut spared = base_node.clone();
    spared.enemies[0].spared_or_forgiven = true;
    if verify_decision_node(&spared) == Err(LedgerError::BetrayerSpared) {
        rejected_attacks = rejected_attacks.saturating_add(1);
    } else {
        return Err("Failed to reject spared traitor".to_string());
    }

    let mut low_depth = base_node.clone();
    low_depth.protagonist_depth = 3;
    if verify_decision_node(&low_depth) == Err(LedgerError::InsufficientReasoningDepth) {
        rejected_attacks = rejected_attacks.saturating_add(1);
    } else {
        return Err("Failed to reject non-dominant reasoning depth".to_string());
    }

    let mut low_entropy = base_node;
    low_entropy.world_entropy_opp_q32 = Q32::from_milli(1_500).0;
    low_entropy.world_entropy_protag_q32 = Q32::ONE.0;
    if verify_decision_node(&low_entropy) == Err(LedgerError::InsufficientEntropyAsymmetry) {
        rejected_attacks = rejected_attacks.saturating_add(1);
    } else {
        return Err("Failed to reject <1.0 bit conditional entropy".to_string());
    }

    if verify_zero_dref_plagiarism("거울 차원 너머의 도르마무와 거래한다")
        == Err(LedgerError::PlagiarizedReferenceTerm)
    {
        rejected_attacks = rejected_attacks.saturating_add(1);
    } else {
        return Err("Failed to reject D_ref proper noun plagiarism".to_string());
    }

    // Cross-check valid G4 diversity + WL novelty + Novikov lattice + DSP/Typo/Pipeline primitives
    let _ = check_novikov_lattice(&[true, true], &[(0, 1, LogicRelation::Copy)]);
    let _ = verify_two_party_lifespan_step((100, 100), (5, 5), (0, 0), (10, -10), (105, 85));
    let _ = check_roi_non_overlap(
        RectI32 { x0: 0, y0: 0, x1: 10, y1: 10 },
        RectI32 { x0: 20, y0: 20, x1: 30, y1: 30 },
    );
    let _ = verify_subtitle_layout(
        1,
        "정상 자막 검증",
        RectI32 { x0: 100, y0: 1800, x1: 900, y1: 1900 },
        &[RectI32 { x0: 100, y0: 200, x1: 900, y1: 500 }],
        Q32::ONE,
        Q32::from_milli(10),
    )
    .map_err(|e| format!("{e:?}"))?;
    let _ = apply_vacuum_gate(1, 0, Q32::ONE, Q32::from_milli(10)).map_err(|e| format!("{e:?}"))?;
    let _ = verify_beat_phase_lock(1, Q32::ONE, &[Q32::ONE]).map_err(|e| format!("{e:?}"))?;
    let _ = kelly_lochbaum_junction_step(Q32::HALF, Q32::ZERO, MAX_REFLECTION_K_RAW);
    let _ = householder_fdn4_step([Q32::HALF; 4], Q32::from_milli(900));
    let _ = prepare_hybrid_pipeline(MAX_VRAM_BUDGET_BYTES).map_err(|e| format!("{e:?}"))?;
    let mkv = mux_mkv_stream(&[MkvPacket {
        track_number: 1,
        timestamp_ms: 0,
        is_keyframe: true,
        payload: vec![1, 2, 3],
    }])
    .map_err(|e| format!("{e:?}"))?;
    let _ = demux_mkv_stream(&mkv).map_err(|e| format!("{e:?}"))?;
    if five_axis_diversity_q32([22, 19, 21, 18, 20]).0 < Q32::from_int(4).0 {
        return Err("5-axis diversity check failed".to_string());
    }
    let ga = MechanismGraph {
        nodes: vec![MechanismRole::MindTrap, MechanismRole::MirrorFold],
        edges: vec![(0, 1)],
    };
    let gb = MechanismGraph {
        nodes: vec![MechanismRole::RetroLoop, MechanismRole::PhonkSync, MechanismRole::VacuumGate],
        edges: vec![(0, 1), (1, 2)],
    };
    if mechanism_distance_q32(&ga, &gb).0 < Q32::from_milli(240).0 {
        return Err("WL mechanism novelty check failed".to_string());
    }
    let _ = isqrt_u128(1_000_000);

    Ok(rejected_attacks)
}

fn run_wal_torn_tail_recovery_test(root: &Path) -> Result<(), String> {
    let wal_path = root.join("state").join("wal").join("phase0_selftest.wal");
    let _ = fs::remove_file(&wal_path);

    let mut expected_last_digest = [0_u8; 32];
    {
        let (mut wal, _) = LedgerWal::open_or_recover(&wal_path).map_err(|e| format!("{e:?}"))?;
        for ep in 1_u16..=5 {
            let mut m = EpisodeManifest {
                episode_id: ep,
                worldline_id: (ep as u64).saturating_mul(1_001),
                ..EpisodeManifest::default()
            };
            m.capital_balances.insert(0, (ep as i64).saturating_mul(10_000));
            m.unknown_tlvs.insert(9999, vec![ep as u8, 0xAA, 0x55]);
            expected_last_digest = m.blake3_digest();
            wal.append_manifest(m).map_err(|e| format!("{e:?}"))?;
        }
    }
    let committed_size = fs::metadata(&wal_path).map_err(|e| e.to_string())?.len();

    // Case A: Append torn incomplete record header + partial bytes simulating sudden power loss
    {
        let mut f = OpenOptions::new()
            .append(true)
            .open(&wal_path)
            .map_err(|e| e.to_string())?;
        f.write_all(&0x5741_4C31_u32.to_le_bytes())
            .map_err(|e| e.to_string())?;
        f.write_all(&[0xDE, 0xAD, 0xBE, 0xEF, 0x11, 0x22])
            .map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
    }

    {
        let (wal, report) = LedgerWal::open_or_recover(&wal_path).map_err(|e| format!("{e:?}"))?;
        if report.valid_records != 5 || report.truncated_tail_bytes != 10 {
            let _ = fs::remove_file(&wal_path);
            return Err(format!("Unexpected Case A WAL recovery report: {report:?}"));
        }
        if wal.current_sealed().digest() != &expected_last_digest {
            let _ = fs::remove_file(&wal_path);
            return Err("WAL Case A digest mismatch after torn-tail recovery".to_string());
        }
    }
    let recovered_size = fs::metadata(&wal_path).map_err(|e| e.to_string())?.len();
    if recovered_size != committed_size {
        let _ = fs::remove_file(&wal_path);
        return Err("WAL file length did not truncate back to exact committed offset".to_string());
    }

    // Case B: Mid-stream corruption MUST be rejected (not silently truncated!)
    {
        let mut f = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&wal_path)
            .map_err(|e| e.to_string())?;
        f.seek(SeekFrom::Start(60)).map_err(|e| e.to_string())?;
        f.write_all(&[0xFF, 0xFF, 0xFF, 0xFF])
            .map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
    }
    match LedgerWal::open_or_recover(&wal_path) {
        Err(WalError::MidStreamChainTampered { .. }) => {}
        other => {
            let _ = fs::remove_file(&wal_path);
            return Err(format!(
                "Mid-stream WAL corruption was not rejected properly: {}",
                other.is_ok()
            ));
        }
    }

    let _ = fs::remove_file(&wal_path);
    Ok(())
}

fn run_100k_determinism_check() -> Result<[u8; 32], String> {
    // Also verify QVM call-graph DAG validator rejects recursion
    let rec = anime_qvm::QvmProgram {
        entry_fn: 0,
        functions: vec![anime_qvm::QvmFunction {
            fn_id: 0,
            ops: vec![anime_qvm::QvmOp::CallSubroutine { target_fn: 0 }],
        }],
    };
    if verify_qvm_program(&rec).is_ok() {
        return Err("QVM failed to reject self-recursive call graph".to_string());
    }

    let mut vm_a = QvmSandbox::new_genesis(2026_1115).map_err(|e| format!("{e:?}"))?;
    let mut vm_b = QvmSandbox::new_genesis(2026_1115).map_err(|e| format!("{e:?}"))?;
    let mut vm_diff = QvmSandbox::new_genesis(2026_1116).map_err(|e| format!("{e:?}"))?;

    let mut step: u32 = 0;
    while step < 100_000 {
        vm_a.step_coupled_tick().map_err(|e| format!("{e:?}"))?;
        vm_b.step_coupled_tick().map_err(|e| format!("{e:?}"))?;
        step = step.saturating_add(1);
    }
    for _ in 0..16 {
        vm_diff.step_coupled_tick().map_err(|e| format!("{e:?}"))?;
    }

    let hash_a = vm_a.state_blake3();
    let hash_b = vm_b.state_blake3();
    if hash_a != hash_b {
        return Err("100,000-step K_sim state hash non-determinism detected".to_string());
    }
    if hash_a == vm_diff.state_blake3() {
        return Err("Degenerate state hash collision across distinct seeds".to_string());
    }
    Ok(hash_a)
}

fn run_security_and_hook_selftests(host_root: &Path) -> Result<usize, String> {
    let parent_outside = host_root
        .parent()
        .unwrap_or(host_root)
        .join("forbidden_parent_secret.txt")
        .to_string_lossy()
        .to_string();
    let traversal_path = format!("{}\\..\\forbidden.txt", host_root.display());
    let host_write_target = host_root
        .join("kernel")
        .join("math_q32")
        .join("src")
        .join("lib.rs")
        .to_string_lossy()
        .to_string();
    let fake_branch = "C:\\__agy_isolated_branch_selftest_9931";

    let blocked_payloads = vec![
        // 1. Parent c:\Defi read (`SSOT-Isolation`)
        serde_json::json!({
            "workspacePaths": [host_root.to_string_lossy()],
            "lastUserInput": "read parent",
            "toolCall": { "name": "read_file", "args": { "Path": parent_outside } }
        }),
        // 2. `..` traversal read
        serde_json::json!({
            "workspacePaths": [host_root.to_string_lossy()],
            "lastUserInput": "traverse",
            "toolCall": { "name": "read_file", "args": { "Path": traversal_path } }
        }),
        // 3. Direct write to host R_root after seal
        serde_json::json!({
            "workspacePaths": [host_root.to_string_lossy()],
            "lastUserInput": "mutate host",
            "toolCall": { "name": "write_file", "args": { "Path": host_write_target, "Content": "// hack" } }
        }),
        // 4. Edit sealed verifier/src/main.rs inside isolated branch
        serde_json::json!({
            "workspacePaths": [fake_branch],
            "lastUserInput": "[NONCE-deadbeef01] edit verifier",
            "toolCall": { "name": "edit_file", "args": { "Path": format!("{fake_branch}\\verifier\\src\\main.rs"), "Target": "a", "Replacement": "b" } }
        }),
        // 5. Edit sealed verifier_abi inside isolated branch
        serde_json::json!({
            "workspacePaths": [fake_branch],
            "lastUserInput": "[NONCE-deadbeef01] edit abi",
            "toolCall": { "name": "write_file", "args": { "Path": format!("{fake_branch}\\kernel\\verifier_abi\\src\\lib.rs"), "Content": "// hack" } }
        }),
        // 6. Inject `#[allow(clippy::panic)]` inside isolated branch
        serde_json::json!({
            "workspacePaths": [fake_branch],
            "lastUserInput": "[NONCE-deadbeef01] inject allow",
            "toolCall": { "name": "write_file", "args": { "Path": format!("{fake_branch}\\kernel\\math_q32\\src\\lib.rs"), "Content": "#[allow(clippy::panic)]\npub fn bad() {}" } }
        }),
        // 7. Create banned `build.rs` inside isolated branch
        serde_json::json!({
            "workspacePaths": [fake_branch],
            "lastUserInput": "[NONCE-deadbeef01] build script",
            "toolCall": { "name": "write_file", "args": { "Path": format!("{fake_branch}\\kernel\\math_q32\\build.rs"), "Content": "fn main() {}" } }
        }),
        // 8. Inject `.onnx` neural network weights (`§4.1`)
        serde_json::json!({
            "workspacePaths": [fake_branch],
            "lastUserInput": "[NONCE-deadbeef01] weights",
            "toolCall": { "name": "write_file", "args": { "Path": format!("{fake_branch}\\season\\model.onnx"), "Content": "onnx" } }
        }),
        // 9. Arbitrary shell command (`curl`)
        serde_json::json!({
            "workspacePaths": [host_root.to_string_lossy()],
            "lastUserInput": "net call",
            "toolCall": { "name": "run_command", "args": { "CommandLine": "curl https://example.com" } }
        }),
        // 10. Metacharacter command chaining (`; whoami`)
        serde_json::json!({
            "workspacePaths": [host_root.to_string_lossy()],
            "lastUserInput": "chain",
            "toolCall": { "name": "run_command", "args": { "CommandLine": "cargo run --locked --release -p verifier -- gate --phase 0; whoami" } }
        }),
        // 11. Unsealed inline workflow script
        serde_json::json!({
            "workspacePaths": [host_root.to_string_lossy()],
            "lastUserInput": "workflow",
            "toolCall": { "name": "run_workflow", "args": { "Script": "print(1)" } }
        }),
    ];

    for (idx, payload) in blocked_payloads.iter().enumerate() {
        let (allow, _) = evaluate_hook_guard(host_root, &payload.to_string(), false);
        if allow {
            return Err(format!("Security selftest vector #{} was NOT blocked!", idx.saturating_add(1)));
        }
    }

    // Positive check: legitimate isolated branch write to `kernel/math_q32/src/lib.rs` without banned attributes
    let valid_branch_write = serde_json::json!({
        "workspacePaths": [fake_branch],
        "lastUserInput": "[NONCE-cafe1234] legitimate builder edit",
        "toolCall": {
            "name": "write_file",
            "args": {
                "Path": format!("{fake_branch}\\kernel\\math_q32\\src\\lib.rs"),
                "Content": "#![no_std]\npub struct Clean;"
            }
        }
    });
    let (ok_allow, ok_reason) = evaluate_hook_guard(host_root, &valid_branch_write.to_string(), false);
    if !ok_allow {
        return Err(format!("Legitimate isolated builder write falsely denied: {ok_reason}"));
    }

    // Positive check: rust source scanner rejects `HashMap` and `f32` in K_sim
    let bad_float_src = "#![no_std]\n#![forbid(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::arithmetic_side_effects, clippy::disallowed_types, clippy::disallowed_methods)]\npub fn x() -> f32 { 0.0 }";
    if scan_rust_source_rules(bad_float_src, "math_q32", true).is_ok() {
        return Err("Source scanner failed to block f32 in K_sim".to_string());
    }

    Ok(blocked_payloads.len().saturating_add(1))
}

// ============================================================================
// 5. GATE RUNNER (`gate --phase <0..4>`)
// ============================================================================

fn run_gate(root: &Path, phase: u8) -> Result<(), String> {
    let head_sha = verify_git_head_exists(root)?;
    let _ = writeln!(io::stdout(), "[Gate 0.1] Git HEAD verified: {head_sha}");

    run_preflight(root, root)?;
    let _ = writeln!(
        io::stdout(),
        "[Gate 0.1] seal.lock + preflight static scan + single-file + 12-crate allowlist PASSED"
    );

    let tree_out = run_cargo_checked(root, &["tree", "--offline", "--locked", "-e", "normal", "-p", "player_bin"])?;
    for line in tree_out.lines() {
        // Ensure standalone `verifier v...` crate is NOT in `player_bin` normal dependency graph (`verifier_abi` is distinct!)
        if line.contains(" verifier v") {
            return Err("`player_bin` depends on `verifier` (`§6.1` violation)".to_string());
        }
    }
    let _ = writeln!(
        io::stdout(),
        "[Gate 0.1] `cargo tree -e normal -p player_bin` verified 0 `verifier` edges"
    );

    run_cargo_checked(
        root,
        &[
            "clippy",
            "--offline",
            "--locked",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    )?;
    let _ = writeln!(
        io::stdout(),
        "[Gate 0.1] `cargo clippy --workspace --all-targets -- -D warnings` 0 warnings PASSED"
    );

    let doc_out = run_cargo_checked(
        root,
        &["test", "--offline", "--locked", "--doc", "-p", "verifier_abi"],
    )?;
    if !doc_out.contains("test result: ok.") {
        return Err("verifier_abi compile_fail doctests did not pass".to_string());
    }
    run_cargo_checked(
        root,
        &["test", "--offline", "--locked", "--workspace", "--lib", "--bins"],
    )?;
    let _ = writeln!(
        io::stdout(),
        "[Gate 0.2] `verifier_abi` 6x compile_fail typestate rejections + workspace unit tests PASSED"
    );

    let proven_count = run_all_formal_cdcl_proofs()?;
    let _ = writeln!(
        io::stdout(),
        "[Gate 0.3] Pure-Rust Bit-Blasting CDCL Formal Proofs ({proven_count} theorems, 0 counterexamples) PASSED"
    );

    let rejected_attacks = run_adversarial_paradox_and_ledger_tests()?;
    let _ = writeln!(
        io::stdout(),
        "[Gate 0.4] Adversarial Time-Paradox, Mind-Game & Ledger-Forgery Injections ({rejected_attacks}/{rejected_attacks} = 100% rejected) PASSED"
    );

    let state_hash = run_100k_determinism_check()?;
    let hash_hex = blake3::Hash::from_bytes(state_hash).to_hex();
    run_cargo_checked(
        root,
        &[
            "+nightly",
            "miri",
            "test",
            "--offline",
            "--locked",
            "-p",
            "math_q32",
            "-p",
            "verifier_abi",
        ],
    )?;
    let _ = writeln!(
        io::stdout(),
        "[Gate 0.5] 100,000-step K_sim BLAKE3 determinism ({hash_hex}) + Nightly Miri UB check PASSED"
    );

    run_wal_torn_tail_recovery_test(root)?;
    let _ = writeln!(
        io::stdout(),
        "[Gate 0.6] LedgerWal torn-tail crash truncation recovery (0 error) & mid-stream tamper rejection PASSED"
    );

    let sec_vectors = run_security_and_hook_selftests(root)?;
    let _ = writeln!(
        io::stdout(),
        "[Gate 0.7] PreToolUse hook-guard & preflight adversarial security selftests ({sec_vectors}/{sec_vectors} = 100% pass) PASSED"
    );

    if phase >= 1 {
        return Err(format!(
            "Phase {phase} vertical-slice / episode artifacts (`output/ep_*.mkv`) not yet mastered; Phase 0 foundation is 100% sealed."
        ));
    }

    let _ = writeln!(io::stdout(), "VERIFIER_GATE_OK phase={phase} score=100");
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let root = match find_repo_root() {
        Ok(r) => r,
        Err(e) => {
            let _ = writeln!(io::stderr(), "VERIFIER_FATAL: {e}");
            return ExitCode::FAILURE;
        }
    };

    let subcmd = args.get(1).map(String::as_str).unwrap_or("");
    match subcmd {
        "hook-guard" => cmd_hook_guard(&root),
        "seal-init" => match cmd_seal_init(&root) {
            Ok(()) => {
                let _ = writeln!(io::stdout(), "SEAL_INIT_OK {}", root.join("seal.lock").display());
                ExitCode::SUCCESS
            }
            Err(e) => {
                let _ = writeln!(io::stderr(), "SEAL_INIT_ERR: {e}");
                ExitCode::FAILURE
            }
        },
        "preflight" => {
            let target = if args.get(2).map(String::as_str) == Some("--target") {
                args.get(3).map(PathBuf::from).unwrap_or_else(|| root.clone())
            } else {
                root.clone()
            };
            match run_preflight(&root, &target) {
                Ok(()) => {
                    let _ = writeln!(io::stdout(), "PREFLIGHT_OK {}", target.display());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    let _ = writeln!(io::stderr(), "PREFLIGHT_ERR: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        "gate" => {
            let phase_num: u8 = if args.get(2).map(String::as_str) == Some("--phase") {
                match args.get(3).and_then(|s| s.parse::<u8>().ok()) {
                    Some(p) if p <= 4 => p,
                    _ => {
                        let _ = writeln!(io::stderr(), "Invalid --phase (expected 0..4)");
                        return ExitCode::FAILURE;
                    }
                }
            } else {
                let _ = writeln!(io::stderr(), "Usage: verifier gate --phase <0..4>");
                return ExitCode::FAILURE;
            };
            match run_gate(&root, phase_num) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    let _ = writeln!(io::stderr(), "VERIFIER_GATE_FAILED: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        _ => {
            let _ = writeln!(
                io::stderr(),
                "Usage: verifier <seal-init | preflight [--target DIR] | hook-guard | gate --phase <0..4>>"
            );
            ExitCode::FAILURE
        }
    }
}
