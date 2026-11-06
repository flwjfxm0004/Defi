lines = open('C:/Defi/인스타.md', encoding='utf-8').read().splitlines()
new_lines = [
 '- [x] Ghidra 12.2 + JDK 25 설치 완료 (ghidra_setup.ps1)',
 '- [~] Ghidra 헤드리스 분석 시도 -> 379MB 바이너리 임포트 실패 (PC 성능 부족). Ghidra GUI로 수동 분석 필요.',
 '- [x] Kaleidoscope PDF/FontFile 검증 파이프라인 식별 (ks_classify, pdfcheck, FontFile/FontFile2/FontFile3, MalformedFontProgram 등)',
 '- [x] Calif CVE-2026-86950 The Great Glyph Grift 분석 -> CoreGraphics 폰트 path rasterization integer overflow, PDF FontFile 통해 도달',
 '- [x] 공격 벡터 확정: (1) Kaleidoscope Rust 폰트/PDF 파싱 버그 -> Instagram RCE; (2) Kaleidoscope 우회 -> CoreGraphics RCE',
]
lines[84:85] = new_lines
open('C:/Defi/인스타.md', 'w', encoding='utf-8').write('\n'.join(lines) + '\n')
print('done')
PYEOF
