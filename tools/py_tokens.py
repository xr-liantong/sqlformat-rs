import sys
from sqlparse import lexer

def esc(v):
    return v.replace("\\", "\\\\").replace("\n", "\\n").replace("\r", "\\r").replace("\t", "\\t")

text = open(sys.argv[1], encoding="utf-8").read()
for tt, value in lexer.tokenize(text):
    print(f"{tt}\t{esc(value)}")
