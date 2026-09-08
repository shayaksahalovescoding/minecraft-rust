from pathlib import Path

p = Path(r"C:\Users\shaya\AppData\Local\Programs\Python\Python312\Lib\site-packages\fairseq\dataclass\initialize.py")
if p.exists():
    content = p.read_text(encoding="utf-8")
    target = """        try:
            cs.store(name=k, node=v)
        except BaseException:
            logger.error(f"{k} - {v}")
            raise"""
    replacement = """        try:
            cs.store(name=k, node=v)
        except BaseException:
            pass"""
    if target in content:
        content = content.replace(target, replacement)
        p.write_text(content, encoding="utf-8")
        print("Fairseq successfully patched for Python 3.12")
    else:
        print("Target pattern not found")
