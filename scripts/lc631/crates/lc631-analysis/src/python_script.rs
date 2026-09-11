pub(crate) const SCRIPT: &str = r#"
import ast
import dis
import hashlib
import json
import marshal
import platform
import symtable
import sys
import sysconfig

source = sys.stdin.read()

def dotted(node):
    if isinstance(node, ast.Name):
        return node.id
    if isinstance(node, ast.Attribute):
        base = dotted(node.value)
        return f"{base}.{node.attr}" if base else node.attr
    return ""

def scopes(table):
    return 1 + sum(scopes(child) for child in table.get_children())

class Inventory(ast.NodeVisitor):
    def __init__(self):
        self.nodes = 0
        self.imports = []
        self.functions = 0
        self.classes = 0
        self.asyncs = 0
        self.annotations = 0
        self.risks = set()
    def generic_visit(self, node):
        self.nodes += 1
        super().generic_visit(node)
    def visit_Import(self, node):
        self.imports.extend(alias.name for alias in node.names)
        self.generic_visit(node)
    def visit_ImportFrom(self, node):
        self.imports.append(node.module or "<relative>")
        self.generic_visit(node)
    def visit_FunctionDef(self, node):
        self.functions += 1
        args = node.args.posonlyargs + node.args.args + node.args.kwonlyargs
        self.annotations += sum(arg.annotation is not None for arg in args)
        self.annotations += int(node.returns is not None)
        self.generic_visit(node)
    def visit_AsyncFunctionDef(self, node):
        self.asyncs += 1
        self.visit_FunctionDef(node)
    def visit_Await(self, node):
        self.asyncs += 1
        self.generic_visit(node)
    def visit_ClassDef(self, node):
        self.classes += 1
        if node.keywords:
            self.risks.add("metaclass_or_dynamic_class")
        self.generic_visit(node)
    def visit_AnnAssign(self, node):
        self.annotations += 1
        self.generic_visit(node)
    def visit_Call(self, node):
        name = dotted(node.func)
        if name in {"eval", "exec", "compile"}:
            self.risks.add("dynamic_code_execution")
        if name in {"__import__", "importlib.import_module"}:
            self.risks.add("dynamic_import")
        if name in {"os.system", "subprocess.call", "subprocess.run", "subprocess.Popen"}:
            self.risks.add("process_or_shell_execution")
        if name in {"pickle.load", "pickle.loads", "marshal.load", "marshal.loads"}:
            self.risks.add("unsafe_deserialization_candidate")
        if name in {"setattr", "delattr"}:
            self.risks.add("runtime_attribute_mutation")
        if name.startswith("ctypes.") or name.startswith("cffi."):
            self.risks.add("native_extension_boundary")
        for keyword in node.keywords:
            if keyword.arg == "shell" and isinstance(keyword.value, ast.Constant) and keyword.value.value is True:
                self.risks.add("shell_true")
        self.generic_visit(node)

gil_probe = getattr(sys, "_is_gil_enabled", None)
jit = getattr(sys, "_jit", None)
jit_probe = getattr(jit, "is_enabled", None) if jit is not None else None
result = {
    "schema": "lc631-python-compiler-observation.v1",
    "interpreter": {
        "implementation": sys.implementation.name,
        "version": ".".join(str(value) for value in sys.version_info[:3]),
        "cache_tag": getattr(sys.implementation, "cache_tag", None),
        "platform": sys.platform,
        "machine": platform.machine(),
        "abi_flags": getattr(sys, "abiflags", ""),
        "sysconfig_platform": sysconfig.get_platform(),
        "virtual_environment": sys.prefix != sys.base_prefix,
        "free_threaded_build": sysconfig.get_config_var("Py_GIL_DISABLED") == 1,
        "gil_enabled": bool(gil_probe()) if callable(gil_probe) else None,
        "jit_enabled": bool(jit_probe()) if callable(jit_probe) else None,
    },
    "parsed": False,
    "compiled": False,
    "syntax_error": None,
    "code_object_digest": None,
    "opcode_digest": None,
    "metrics": {},
    "imports": [],
    "risks": [],
}
try:
    tree = ast.parse(source, filename="<lc631-python-unit>", mode="exec", type_comments=True)
    code = compile(tree, "<lc631-python-unit>", "exec", dont_inherit=True, optimize=0)
    table = symtable.symtable(source, "<lc631-python-unit>", "exec")
    instructions = list(dis.get_instructions(code, adaptive=False))
    opnames = [instruction.opname for instruction in instructions]
    inventory = Inventory()
    inventory.visit(tree)
    passive = (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef, ast.Import, ast.ImportFrom)
    result["parsed"] = True
    result["compiled"] = True
    result["code_object_digest"] = hashlib.sha256(marshal.dumps(code)).hexdigest()
    result["opcode_digest"] = hashlib.sha256("\n".join(opnames).encode("utf-8")).hexdigest()
    result["metrics"] = {
        "ast_nodes": inventory.nodes,
        "scopes": scopes(table),
        "imports": len(inventory.imports),
        "functions": inventory.functions,
        "classes": inventory.classes,
        "async_constructs": inventory.asyncs,
        "annotations": inventory.annotations,
        "top_level_effects": sum(not isinstance(node, passive) for node in tree.body),
        "bytecode_instructions": len(instructions),
        "distinct_opcodes": len(set(opnames)),
    }
    result["imports"] = sorted(set(inventory.imports))
    result["risks"] = sorted(inventory.risks)
except SyntaxError as error:
    result["syntax_error"] = f"{error.msg} at {error.lineno}:{error.offset}"
print(json.dumps(result, sort_keys=True, separators=(",", ":")))
"#;
