//! Registry of implemented `node:` builtins and their served ESM sources.

/// One implemented builtin: its name and the exported member list.
pub(crate) struct NodeShim {
    pub name: &'static str,
    /// Comma-separated members exposed as ESM named exports.
    pub exports: &'static str,
}

/// The supported subset; anything else reports "not implemented on Bap".
pub(crate) const NODE_SHIMS: &[NodeShim] = &[
    NodeShim {
        name: "path",
        exports: "normalize,join,resolve,isAbsolute,dirname,basename,extname,relative,sep,delimiter",
    },
    NodeShim {
        name: "fs",
        exports: "readFileSync,writeFileSync,existsSync,statSync,readdirSync,mkdirSync,promises,constants",
    },
    NodeShim {
        name: "util",
        exports: "format,inspect,promisify,deprecate,isArray,inherits",
    },
    NodeShim {
        name: "events",
        exports: "EventEmitter,defaultMaxListeners,once",
    },
    NodeShim {
        name: "buffer",
        exports: "Buffer",
    },
    NodeShim {
        name: "process",
        exports: "env,argv,cwd,exit,nextTick,version,versions,platform,arch",
    },
];

/// URL scheme the resolver maps `node:<name>` specifiers onto.
pub(crate) const SHIM_SCHEME: &str = "bap-shim";

pub(crate) fn find(name: &str) -> Option<&'static NodeShim> {
    NODE_SHIMS.iter().find(|shim| shim.name == name)
}

/// Builds the synthetic ESM re-exporting the extension-installed namespace.
pub(crate) fn module_source(name: &str) -> String {
    let shim = find(name).expect("registered shim");
    let mut code = String::with_capacity(256);
    let lookup = serde_json::to_string(shim.name).expect("json string");
    code.push_str(&format!(
        "const ns = globalThis.__bapNodeModules[{lookup}];\n"
    ));
    code.push_str(&format!(
        "if (!ns) throw new Error(\"bap: node:{name} namespace unavailable\");\n"
    ));
    code.push_str("export default ns;\n");
    for member in shim.exports.split(',') {
        code.push_str(&format!(
            "const {member} = ns.{member};\nexport {{ {member} }};\n"
        ));
    }
    code
}
