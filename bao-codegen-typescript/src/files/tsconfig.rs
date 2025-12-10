//! tsconfig.json generator for TypeScript projects.

use std::path::{Path, PathBuf};

use baobao_core::{FileRules, GeneratedFile, Overwrite};

/// The tsconfig.json configuration file.
pub struct TsConfig;

impl GeneratedFile for TsConfig {
    fn path(&self, base: &Path) -> PathBuf {
        base.join("tsconfig.json")
    }

    fn rules(&self) -> FileRules {
        FileRules {
            overwrite: Overwrite::IfMissing,
            header: None,
        }
    }

    fn render(&self) -> String {
        r#"{
  "compilerOptions": {
    "lib": ["ESNext"],
    "target": "ESNext",
    "module": "ESNext",
    "moduleDetection": "force",
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "verbatimModuleSyntax": true,
    "noEmit": true,
    "strict": true,
    "skipLibCheck": true,
    "noFallthroughCasesInSwitch": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noPropertyAccessFromIndexSignature": true,
    "resolveJsonModule": true,
    "esModuleInterop": true
  },
  "include": ["src/**/*.ts"]
}
"#
        .to_string()
    }
}
