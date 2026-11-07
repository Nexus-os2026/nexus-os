"""PR-1 inert interfaces: contract checkpoint must be red."""
LIMITS = {"seconds": 300, "entries": 100000, "objects": 100000, "bytes": 3*1024**3, "export": 2*1024**3, "blob": 64*1024**2, "tree": 8*1024**2, "commit": 1024**2, "depth": 64, "path": 4096, "component": 255}
class GitSource:
    def __init__(self, path, device, inode):
        raise NotImplementedError("PR-1 inert GitSource")

def parse_request(frame, registry, uid):
    raise NotImplementedError("PR-1 inert interface")

def export(source, sha, dest):
    raise NotImplementedError("PR-1 inert interface")

def validate_path(name):
    raise NotImplementedError("PR-1 inert interface")

def validate_link(name, target):
    raise NotImplementedError("PR-1 inert interface")

def manifest(root):
    raise NotImplementedError("PR-1 inert interface")

