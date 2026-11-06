# Ghidra headless post-analysis script
# Usage: analyzeHeadless <project> <name> -process Instagram -postScript ghidra_extract.py
# Extracts functions/classes of interest for zero-click RCE research.

from ghidra.program.model.symbol import SourceType

prog = currentProgram
fm = prog.getFunctionManager()
names = ["Image", "Media", "WebP", "Webp", "PNG", "MP4", "HEIC", "HEIF", "PDF", "Decoder", "Parser", "CGImage"]

out = open("C:/Defi/research/ghidra_functions.txt", "w")
count = 0
for f in fm.getFunctions(True):
    n = f.getName()
    if any(k.lower() in n.lower() for k in names):
        out.write("%s @ %s\n" % (n, f.getEntryPoint()))
        count += 1
out.close()
print("Extracted %d functions" % count)
