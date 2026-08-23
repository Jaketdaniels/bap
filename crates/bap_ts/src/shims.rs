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
        name: "assert",
        exports: "AssertionError,ok,fail,equal,notEqual,strictEqual,notStrictEqual,deepEqual,deepStrictEqual,throws,rejects,doesNotThrow,ifError",
    },
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
        exports: "env,argv,cwd,exit,nextTick,stdout,version,versions,platform,arch",
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

/// Public names of the implemented builtins, kept for adoption tooling.
pub static IMPLEMENTED_NODE_BUILTINS: &[&str] = &[
    "assert", "path", "fs", "util", "events", "buffer", "process",
];

/// Every builtin name Node reserves; members outside IMPLEMENTED report
/// "not implemented" instead of "missing npm package".
pub static KNOWN_NODE_BUILTINS: &[&str] = &[
    "assert",
    "async_hooks",
    "buffer",
    "child_process",
    "cluster",
    "console",
    "constants",
    "crypto",
    "dgram",
    "diagnostics_channel",
    "dns",
    "domain",
    "events",
    "fs",
    "http",
    "http2",
    "https",
    "inspector",
    "module",
    "net",
    "os",
    "path",
    "perf_hooks",
    "process",
    "punycode",
    "querystring",
    "readline",
    "repl",
    "stream",
    "string_decoder",
    "sys",
    "timers",
    "tls",
    "trace_events",
    "tty",
    "url",
    "util",
    "v8",
    "vm",
    "worker_threads",
    "zlib",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_builtin_list_matches_registry() {
        let registry: Vec<&'static str> = NODE_SHIMS.iter().map(|shim| shim.name).collect();
        assert_eq!(registry, IMPLEMENTED_NODE_BUILTINS.to_vec());
    }

    #[test]
    fn every_implemented_builtin_is_known() {
        assert!(
            IMPLEMENTED_NODE_BUILTINS
                .iter()
                .all(|name| KNOWN_NODE_BUILTINS.contains(name))
        );
    }
}
