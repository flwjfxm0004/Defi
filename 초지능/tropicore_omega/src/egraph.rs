//! Real Union-Find E-Graph Equality Saturation Engine with Live Lean-4 Proof-to-Rewrite Promotion!
//!
//! Key First-Principles Synergy:
//!   Every algebraic equality theorem proved by `prover.rs` inside live `Lean 4.36.0` is
//!   automatically compiled into a verified `RewriteRule` (`Pattern -> Pattern`) and installed
//!   into this `EGraph`! Proving math literally strengthens the engine's own compiler & synthesizer!

use std::collections::HashMap;

use crate::ir::OmegaExpr;

pub type EClassId = usize;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum ENode {
    Arg(usize),
    ConstBits(u64), // quantized 1e-6 f64 key
    Param(usize),
    Add(EClassId, EClassId),
    Sub(EClassId, EClassId),
    Mul(EClassId, EClassId),
    DivSafe(EClassId, EClassId),
    Min(EClassId, EClassId),
    Max(EClassId, EClassId),
    Abs(EClassId),
    SqrtSafe(EClassId),
    IfPos(EClassId, EClassId, EClassId),
}

/// First-order pattern AST for verified rewrite rules (metavariables `Var(0), Var(1), ...`).
#[derive(Clone, Debug, PartialEq)]
pub enum Pattern {
    Var(usize),
    Const(i64),
    Add(Box<Pattern>, Box<Pattern>),
    Sub(Box<Pattern>, Box<Pattern>),
    Mul(Box<Pattern>, Box<Pattern>),
    Min(Box<Pattern>, Box<Pattern>),
    Max(Box<Pattern>, Box<Pattern>),
    Abs(Box<Pattern>),
}

#[derive(Clone, Debug)]
pub struct RewriteRule {
    pub name: String,
    pub lhs: Pattern,
    pub rhs: Pattern,
    pub proved_by_lean4: bool,
}

impl Pattern {
    /// Convert a pure integer `OmegaExpr` (`Arg(i)` -> metavariable `Var(i)`) into a `Pattern`.
    pub fn from_omega_expr(e: &OmegaExpr) -> Option<Self> {
        match e {
            OmegaExpr::Arg(i) => Some(Pattern::Var(*i)),
            OmegaExpr::Const(c) if (c - c.round()).abs() < 1e-6 => Some(Pattern::Const(c.round() as i64)),
            OmegaExpr::Add(a, b) => Some(Pattern::Add(Box::new(Self::from_omega_expr(a)?), Box::new(Self::from_omega_expr(b)?))),
            OmegaExpr::Sub(a, b) => Some(Pattern::Sub(Box::new(Self::from_omega_expr(a)?), Box::new(Self::from_omega_expr(b)?))),
            OmegaExpr::Mul(a, b) => Some(Pattern::Mul(Box::new(Self::from_omega_expr(a)?), Box::new(Self::from_omega_expr(b)?))),
            OmegaExpr::Min(a, b) => Some(Pattern::Min(Box::new(Self::from_omega_expr(a)?), Box::new(Self::from_omega_expr(b)?))),
            OmegaExpr::Max(a, b) => Some(Pattern::Max(Box::new(Self::from_omega_expr(a)?), Box::new(Self::from_omega_expr(b)?))),
            OmegaExpr::Abs(a) => Some(Pattern::Abs(Box::new(Self::from_omega_expr(a)?))),
            _ => None,
        }
    }

    /// Bit-exact S-expression serializer for lossless full-brain persistence of verified E-Graph rules (`v7.0`).
    pub fn to_sexp(&self) -> String {
        match self {
            Pattern::Var(i) => format!("(v {})", i),
            Pattern::Const(c) => format!("(k {})", c),
            Pattern::Add(a, b) => format!("(+ {} {})", a.to_sexp(), b.to_sexp()),
            Pattern::Sub(a, b) => format!("(- {} {})", a.to_sexp(), b.to_sexp()),
            Pattern::Mul(a, b) => format!("(* {} {})", a.to_sexp(), b.to_sexp()),
            Pattern::Min(a, b) => format!("(min {} {})", a.to_sexp(), b.to_sexp()),
            Pattern::Max(a, b) => format!("(max {} {})", a.to_sexp(), b.to_sexp()),
            Pattern::Abs(a) => format!("(abs {})", a.to_sexp()),
        }
    }

    /// Bit-exact S-expression parser for verified E-Graph rules (`v7.0`).
    pub fn from_sexp(s: &str) -> Option<Self> {
        let mut toks: Vec<String> = Vec::new();
        let mut cur = String::new();
        for ch in s.chars() {
            match ch {
                '(' | ')' => {
                    if !cur.is_empty() {
                        toks.push(std::mem::take(&mut cur));
                    }
                    toks.push(ch.to_string());
                }
                c if c.is_whitespace() => {
                    if !cur.is_empty() {
                        toks.push(std::mem::take(&mut cur));
                    }
                }
                _ => cur.push(ch),
            }
        }
        if !cur.is_empty() {
            toks.push(cur);
        }
        fn parse(toks: &[String], pos: &mut usize) -> Option<Pattern> {
            if toks.get(*pos)?.as_str() != "(" {
                return None;
            }
            *pos += 1;
            let op = toks.get(*pos)?.as_str();
            *pos += 1;
            let res = match op {
                "v" => {
                    let i: usize = toks.get(*pos)?.parse().ok()?;
                    *pos += 1;
                    Pattern::Var(i)
                }
                "k" => {
                    let c: i64 = toks.get(*pos)?.parse().ok()?;
                    *pos += 1;
                    Pattern::Const(c)
                }
                "+" => Pattern::Add(Box::new(parse(toks, pos)?), Box::new(parse(toks, pos)?)),
                "-" => Pattern::Sub(Box::new(parse(toks, pos)?), Box::new(parse(toks, pos)?)),
                "*" => Pattern::Mul(Box::new(parse(toks, pos)?), Box::new(parse(toks, pos)?)),
                "min" => Pattern::Min(Box::new(parse(toks, pos)?), Box::new(parse(toks, pos)?)),
                "max" => Pattern::Max(Box::new(parse(toks, pos)?), Box::new(parse(toks, pos)?)),
                "abs" => Pattern::Abs(Box::new(parse(toks, pos)?)),
                _ => return None,
            };
            if toks.get(*pos)?.as_str() != ")" {
                return None;
            }
            *pos += 1;
            Some(res)
        }
        let mut pos = 0usize;
        let p = parse(&toks, &mut pos)?;
        if pos == toks.len() { Some(p) } else { None }
    }
}

fn f64_to_key(c: f64) -> u64 {
    ((c * 1e6).round() as i64) as u64
}

fn key_to_f64(k: u64) -> f64 {
    (k as i64) as f64 / 1e6
}

#[derive(Clone, Debug)]
struct EClass {
    nodes: Vec<ENode>,
}

/// Real Union-Find + Hashcons E-Graph supporting bidirectional verified rewrite saturation & MDL extraction.
#[derive(Clone, Debug)]
pub struct EGraph {
    parent: Vec<EClassId>,
    classes: Vec<EClass>,
    hashcons: HashMap<ENode, EClassId>,
}

impl EGraph {
    pub fn new() -> Self {
        Self {
            parent: Vec::new(),
            classes: Vec::new(),
            hashcons: HashMap::new(),
        }
    }

    pub fn find(&self, mut id: EClassId) -> EClassId {
        while self.parent[id] != id {
            id = self.parent[id];
        }
        id
    }

    pub fn find_mut(&mut self, id: EClassId) -> EClassId {
        if self.parent[id] == id {
            return id;
        }
        let root = self.find_mut(self.parent[id]);
        self.parent[id] = root;
        root
    }

    fn canonicalize_node(&mut self, n: ENode) -> ENode {
        match n {
            ENode::Add(a, b) => ENode::Add(self.find_mut(a), self.find_mut(b)),
            ENode::Sub(a, b) => ENode::Sub(self.find_mut(a), self.find_mut(b)),
            ENode::Mul(a, b) => ENode::Mul(self.find_mut(a), self.find_mut(b)),
            ENode::DivSafe(a, b) => ENode::DivSafe(self.find_mut(a), self.find_mut(b)),
            ENode::Min(a, b) => ENode::Min(self.find_mut(a), self.find_mut(b)),
            ENode::Max(a, b) => ENode::Max(self.find_mut(a), self.find_mut(b)),
            ENode::Abs(a) => ENode::Abs(self.find_mut(a)),
            ENode::SqrtSafe(a) => ENode::SqrtSafe(self.find_mut(a)),
            ENode::IfPos(c, t, e) => ENode::IfPos(self.find_mut(c), self.find_mut(t), self.find_mut(e)),
            other => other,
        }
    }

    pub fn add_node(&mut self, node: ENode) -> EClassId {
        let cn = self.canonicalize_node(node);
        if let Some(&id) = self.hashcons.get(&cn) {
            return self.find_mut(id);
        }
        let id = self.parent.len();
        self.parent.push(id);
        self.classes.push(EClass { nodes: vec![cn.clone()] });
        self.hashcons.insert(cn, id);
        id
    }

    pub fn union(&mut self, a: EClassId, b: EClassId) -> bool {
        let ra = self.find_mut(a);
        let rb = self.find_mut(b);
        if ra == rb {
            return false;
        }
        let (keep, merge) = if ra < rb { (ra, rb) } else { (rb, ra) };
        self.parent[merge] = keep;
        let moved = std::mem::take(&mut self.classes[merge].nodes);
        for n in moved {
            if !self.classes[keep].nodes.contains(&n) {
                self.classes[keep].nodes.push(n);
            }
        }
        true
    }

    pub fn rebuild(&mut self) {
        let mut changed = true;
        let mut guard = 0;
        while changed && guard < 16 {
            changed = false;
            guard += 1;
            let mut new_hc: HashMap<ENode, EClassId> = HashMap::new();
            let mut merges: Vec<(EClassId, EClassId)> = Vec::new();
            for cid in 0..self.classes.len() {
                if self.parent[cid] != cid {
                    continue;
                }
                let nodes = std::mem::take(&mut self.classes[cid].nodes);
                let mut deduped = Vec::with_capacity(nodes.len());
                for n in nodes {
                    let cn = self.canonicalize_node(n);
                    if let Some(&existing) = new_hc.get(&cn) {
                        if self.find(existing) != self.find(cid) {
                            merges.push((existing, cid));
                        }
                    } else {
                        new_hc.insert(cn.clone(), cid);
                    }
                    if !deduped.contains(&cn) {
                        deduped.push(cn);
                    }
                }
                self.classes[cid].nodes = deduped;
            }
            self.hashcons = new_hc;
            for (a, b) in merges {
                if self.union(a, b) {
                    changed = true;
                }
            }
        }
    }

    pub fn add_expr(&mut self, expr: &OmegaExpr) -> Option<EClassId> {
        match expr {
            OmegaExpr::Arg(i) => Some(self.add_node(ENode::Arg(*i))),
            OmegaExpr::Const(c) => Some(self.add_node(ENode::ConstBits(f64_to_key(*c)))),
            OmegaExpr::Param(k) => Some(self.add_node(ENode::Param(*k))),
            OmegaExpr::Add(a, b) => {
                let ia = self.add_expr(a)?;
                let ib = self.add_expr(b)?;
                Some(self.add_node(ENode::Add(ia, ib)))
            }
            OmegaExpr::Sub(a, b) => {
                let ia = self.add_expr(a)?;
                let ib = self.add_expr(b)?;
                Some(self.add_node(ENode::Sub(ia, ib)))
            }
            OmegaExpr::Mul(a, b) => {
                let ia = self.add_expr(a)?;
                let ib = self.add_expr(b)?;
                Some(self.add_node(ENode::Mul(ia, ib)))
            }
            OmegaExpr::DivSafe(a, b) => {
                let ia = self.add_expr(a)?;
                let ib = self.add_expr(b)?;
                Some(self.add_node(ENode::DivSafe(ia, ib)))
            }
            OmegaExpr::Min(a, b) => {
                let ia = self.add_expr(a)?;
                let ib = self.add_expr(b)?;
                Some(self.add_node(ENode::Min(ia, ib)))
            }
            OmegaExpr::Max(a, b) => {
                let ia = self.add_expr(a)?;
                let ib = self.add_expr(b)?;
                Some(self.add_node(ENode::Max(ia, ib)))
            }
            OmegaExpr::Abs(a) => {
                let ia = self.add_expr(a)?;
                Some(self.add_node(ENode::Abs(ia)))
            }
            OmegaExpr::SqrtSafe(a) => {
                let ia = self.add_expr(a)?;
                Some(self.add_node(ENode::SqrtSafe(ia)))
            }
            OmegaExpr::IfPos(c, t, e) => {
                let ic = self.add_expr(c)?;
                let it = self.add_expr(t)?;
                let ie = self.add_expr(e)?;
                Some(self.add_node(ENode::IfPos(ic, it, ie)))
            }
            _ => None,
        }
    }

    fn match_pat(&self, pat: &Pattern, cid: EClassId, env: &HashMap<usize, EClassId>) -> Vec<HashMap<usize, EClassId>> {
        let rc = self.find(cid);
        match pat {
            Pattern::Var(v) => {
                if let Some(&bound) = env.get(v) {
                    if self.find(bound) == rc {
                        vec![env.clone()]
                    } else {
                        Vec::new()
                    }
                } else {
                    let mut next = env.clone();
                    next.insert(*v, rc);
                    vec![next]
                }
            }
            Pattern::Const(k) => {
                let target = f64_to_key(*k as f64);
                if self.classes[rc].nodes.iter().any(|n| matches!(n, ENode::ConstBits(b) if *b == target)) {
                    vec![env.clone()]
                } else {
                    Vec::new()
                }
            }
            Pattern::Abs(pa) => {
                let mut out = Vec::new();
                for n in &self.classes[rc].nodes {
                    if let ENode::Abs(a) = *n {
                        out.extend(self.match_pat(pa, a, env));
                    }
                }
                out
            }
            Pattern::Add(pa, pb)
            | Pattern::Sub(pa, pb)
            | Pattern::Mul(pa, pb)
            | Pattern::Min(pa, pb)
            | Pattern::Max(pa, pb) => {
                let mut out = Vec::new();
                for n in &self.classes[rc].nodes {
                    let matched_pair = match (pat, n) {
                        (Pattern::Add(_, _), ENode::Add(a, b)) => Some((*a, *b)),
                        (Pattern::Sub(_, _), ENode::Sub(a, b)) => Some((*a, *b)),
                        (Pattern::Mul(_, _), ENode::Mul(a, b)) => Some((*a, *b)),
                        (Pattern::Min(_, _), ENode::Min(a, b)) => Some((*a, *b)),
                        (Pattern::Max(_, _), ENode::Max(a, b)) => Some((*a, *b)),
                        _ => None,
                    };
                    if let Some((a, b)) = matched_pair {
                        for e1 in self.match_pat(pa, a, env) {
                            out.extend(self.match_pat(pb, b, &e1));
                            if out.len() >= 16 {
                                return out;
                            }
                        }
                    }
                }
                out
            }
        }
    }

    fn instantiate_pat(&mut self, pat: &Pattern, env: &HashMap<usize, EClassId>) -> Option<EClassId> {
        match pat {
            Pattern::Var(v) => env.get(v).copied().map(|id| self.find_mut(id)),
            Pattern::Const(k) => Some(self.add_node(ENode::ConstBits(f64_to_key(*k as f64)))),
            Pattern::Add(a, b) => {
                let ia = self.instantiate_pat(a, env)?;
                let ib = self.instantiate_pat(b, env)?;
                Some(self.add_node(ENode::Add(ia, ib)))
            }
            Pattern::Sub(a, b) => {
                let ia = self.instantiate_pat(a, env)?;
                let ib = self.instantiate_pat(b, env)?;
                Some(self.add_node(ENode::Sub(ia, ib)))
            }
            Pattern::Mul(a, b) => {
                let ia = self.instantiate_pat(a, env)?;
                let ib = self.instantiate_pat(b, env)?;
                Some(self.add_node(ENode::Mul(ia, ib)))
            }
            Pattern::Min(a, b) => {
                let ia = self.instantiate_pat(a, env)?;
                let ib = self.instantiate_pat(b, env)?;
                Some(self.add_node(ENode::Min(ia, ib)))
            }
            Pattern::Max(a, b) => {
                let ia = self.instantiate_pat(a, env)?;
                let ib = self.instantiate_pat(b, env)?;
                Some(self.add_node(ENode::Max(ia, ib)))
            }
            Pattern::Abs(a) => {
                let ia = self.instantiate_pat(a, env)?;
                Some(self.add_node(ENode::Abs(ia)))
            }
        }
    }

    /// Run Equality Saturation with `rules` (including Lean-4-proved theorems!) up to `iters` or `node_limit`.
    pub fn saturate(&mut self, rules: &[RewriteRule], iters: usize, node_limit: usize) -> usize {
        let mut total_unions = 0usize;
        for _ in 0..iters {
            if self.classes.len() >= node_limit {
                break;
            }
            let mut pending: Vec<(EClassId, Pattern, HashMap<usize, EClassId>)> = Vec::new();
            let n_classes = self.classes.len();
            for cid in 0..n_classes {
                if self.parent[cid] != cid {
                    continue;
                }
                for r in rules {
                    let empty = HashMap::new();
                    for env in self.match_pat(&r.lhs, cid, &empty) {
                        pending.push((cid, r.rhs.clone(), env));
                        if pending.len() >= 1024 {
                            break;
                        }
                    }
                }
            }
            let mut round_unions = 0usize;
            for (cid, rhs, env) in pending {
                if self.classes.len() >= node_limit {
                    break;
                }
                if let Some(new_id) = self.instantiate_pat(&rhs, &env) {
                    if self.union(cid, new_id) {
                        round_unions += 1;
                    }
                }
            }
            self.rebuild();
            total_unions += round_unions;
            if round_unions == 0 {
                break;
            }
        }
        total_unions
    }

    /// Extract the globally minimum-MDL `OmegaExpr` in `root`'s equivalence class!
    pub fn extract_min_mdl(&self, root: EClassId) -> Option<OmegaExpr> {
        let n = self.classes.len();
        let mut best_cost: Vec<usize> = vec![usize::MAX / 4; n];
        let mut best_node: Vec<Option<ENode>> = vec![None; n];

        for _ in 0..24 {
            let mut changed = false;
            for cid in 0..n {
                if self.parent[cid] != cid {
                    continue;
                }
                for node in &self.classes[cid].nodes {
                    let c = match *node {
                        ENode::Arg(_) | ENode::ConstBits(_) => 1,
                        ENode::Param(_) => 2,
                        ENode::Abs(a) | ENode::SqrtSafe(a) => 1usize.saturating_add(best_cost[self.find(a)]),
                        ENode::Add(a, b)
                        | ENode::Sub(a, b)
                        | ENode::Mul(a, b)
                        | ENode::DivSafe(a, b)
                        | ENode::Min(a, b)
                        | ENode::Max(a, b) => 1usize
                            .saturating_add(best_cost[self.find(a)])
                            .saturating_add(best_cost[self.find(b)]),
                        ENode::IfPos(c, t, e) => 1usize
                            .saturating_add(best_cost[self.find(c)])
                            .saturating_add(best_cost[self.find(t)])
                            .saturating_add(best_cost[self.find(e)]),
                    };
                    if c < best_cost[cid] {
                        best_cost[cid] = c;
                        best_node[cid] = Some(node.clone());
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }

        fn build(cid: EClassId, eg: &EGraph, best_node: &[Option<ENode>], depth: usize) -> Option<OmegaExpr> {
            if depth > 32 {
                return None;
            }
            let r = eg.find(cid);
            match best_node[r].as_ref()? {
                ENode::Arg(i) => Some(OmegaExpr::Arg(*i)),
                ENode::ConstBits(b) => Some(OmegaExpr::Const(key_to_f64(*b))),
                ENode::Param(k) => Some(OmegaExpr::Param(*k)),
                ENode::Add(a, b) => Some(OmegaExpr::Add(Box::new(build(*a, eg, best_node, depth + 1)?), Box::new(build(*b, eg, best_node, depth + 1)?))),
                ENode::Sub(a, b) => Some(OmegaExpr::Sub(Box::new(build(*a, eg, best_node, depth + 1)?), Box::new(build(*b, eg, best_node, depth + 1)?))),
                ENode::Mul(a, b) => Some(OmegaExpr::Mul(Box::new(build(*a, eg, best_node, depth + 1)?), Box::new(build(*b, eg, best_node, depth + 1)?))),
                ENode::DivSafe(a, b) => Some(OmegaExpr::DivSafe(Box::new(build(*a, eg, best_node, depth + 1)?), Box::new(build(*b, eg, best_node, depth + 1)?))),
                ENode::Min(a, b) => Some(OmegaExpr::Min(Box::new(build(*a, eg, best_node, depth + 1)?), Box::new(build(*b, eg, best_node, depth + 1)?))),
                ENode::Max(a, b) => Some(OmegaExpr::Max(Box::new(build(*a, eg, best_node, depth + 1)?), Box::new(build(*b, eg, best_node, depth + 1)?))),
                ENode::Abs(a) => Some(OmegaExpr::Abs(Box::new(build(*a, eg, best_node, depth + 1)?))),
                ENode::SqrtSafe(a) => Some(OmegaExpr::SqrtSafe(Box::new(build(*a, eg, best_node, depth + 1)?))),
                ENode::IfPos(c, t, e) => Some(OmegaExpr::IfPos(
                    Box::new(build(*c, eg, best_node, depth + 1)?),
                    Box::new(build(*t, eg, best_node, depth + 1)?),
                    Box::new(build(*e, eg, best_node, depth + 1)?),
                )),
            }
        }
        build(root, self, &best_node, 0)
    }

    /// Minimal base syntactic rules (commutativity of Add/Min/Max + additive zero).
    /// Non-trivial cross-op algebraic rules (e.g. `max-min = abs`, `min+max = +`) MUST come from Lean 4 proofs!
    pub fn base_comm_rules() -> Vec<RewriteRule> {
        vec![
            RewriteRule {
                name: "base_add_comm".to_string(),
                lhs: Pattern::Add(Box::new(Pattern::Var(0)), Box::new(Pattern::Var(1))),
                rhs: Pattern::Add(Box::new(Pattern::Var(1)), Box::new(Pattern::Var(0))),
                proved_by_lean4: false,
            },
            RewriteRule {
                name: "base_min_comm".to_string(),
                lhs: Pattern::Min(Box::new(Pattern::Var(0)), Box::new(Pattern::Var(1))),
                rhs: Pattern::Min(Box::new(Pattern::Var(1)), Box::new(Pattern::Var(0))),
                proved_by_lean4: false,
            },
            RewriteRule {
                name: "base_max_comm".to_string(),
                lhs: Pattern::Max(Box::new(Pattern::Var(0)), Box::new(Pattern::Var(1))),
                rhs: Pattern::Max(Box::new(Pattern::Var(1)), Box::new(Pattern::Var(0))),
                proved_by_lean4: false,
            },
        ]
    }
}

/// Optimize `expr` with an E-Graph saturated by `rules` and return the minimal-MDL equivalent AST.
pub fn egraph_optimize(expr: &OmegaExpr, rules: &[RewriteRule]) -> (OmegaExpr, usize) {
    let mut eg = EGraph::new();
    let Some(root) = eg.add_expr(expr) else {
        return (expr.clone(), 0);
    };
    let unions = eg.saturate(rules, 6, 4096);
    let best = eg.extract_min_mdl(root).unwrap_or_else(|| expr.clone());
    (best, unions)
}
