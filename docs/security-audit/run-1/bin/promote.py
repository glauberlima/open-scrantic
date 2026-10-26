"""Trusted parent-side promotion: python3 -I promote.py <agent-root> <per-file-max> <total-max> <rel-path>...
Copies allowlisted regular files from <agent-root>/scratch to <agent-root>/artifacts with descriptor-relative,
no-follow traversal, link-count and size checks before, during and after the copy."""
import os, stat, sys
NF = os.O_NOFOLLOW | os.O_CLOEXEC
def parts(rel):
    if not rel or rel.startswith("/"): raise ValueError("absolute or empty")
    p = rel.split("/")
    if any(c in ("", ".", "..") for c in p): raise ValueError("bad component")
    return p
def walk(root_fd, comps, create):
    fd = os.dup(root_fd)
    for c in comps:
        try:
            nfd = os.open(c, os.O_RDONLY | os.O_DIRECTORY | NF, dir_fd=fd)
        except FileNotFoundError:
            if not create: os.close(fd); raise
            os.mkdir(c, 0o700, dir_fd=fd)
            nfd = os.open(c, os.O_RDONLY | os.O_DIRECTORY | NF, dir_fd=fd)
        if not stat.S_ISDIR(os.fstat(nfd).st_mode): raise ValueError("not dir")
        os.close(fd); fd = nfd
    return fd
root = sys.argv[1]; per = int(sys.argv[2]); tot = int(sys.argv[3]); used = 0
s_root = os.open(os.path.join(root, "scratch"), os.O_RDONLY | os.O_DIRECTORY | NF)
a_root = os.open(os.path.join(root, "artifacts"), os.O_RDONLY | os.O_DIRECTORY | NF)
for rel in sys.argv[4:]:
    try:
        p = parts(rel)
        sd = walk(s_root, p[:-1], False)
        src = os.open(p[-1], os.O_RDONLY | os.O_NONBLOCK | NF, dir_fd=sd)
        st = os.fstat(src)
        if not stat.S_ISREG(st.st_mode) or st.st_nlink != 1 or st.st_size > per or used + st.st_size > tot:
            raise ValueError("type/link/size check failed")
        dd = walk(a_root, p[:-1], True)
        dst = os.open(p[-1], os.O_WRONLY | os.O_CREAT | os.O_EXCL | NF, 0o600, dir_fd=dd)
        dt = os.fstat(dst)
        if not stat.S_ISREG(dt.st_mode) or dt.st_nlink != 1: raise ValueError("dest check failed")
        left = st.st_size
        while left:
            b = os.read(src, min(65536, left))
            if not b: raise ValueError("short read")
            os.write(dst, b); left -= len(b)
        if os.read(src, 1): raise ValueError("file grew")
        st2 = os.fstat(src)
        if (st2.st_ino, st2.st_dev, st2.st_nlink, st2.st_size, stat.S_IFMT(st2.st_mode)) != (st.st_ino, st.st_dev, st.st_nlink, st.st_size, stat.S_IFMT(st.st_mode)):
            raise ValueError("source changed")
        used += st.st_size
        print(f"PROMOTED agents/{os.path.basename(root)}/artifacts/{rel} ({st.st_size} bytes)")
    except Exception as e:
        print(f"REJECTED {rel}: {e}")
