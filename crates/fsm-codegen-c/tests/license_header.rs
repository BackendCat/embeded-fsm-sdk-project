//! License-header test — Doc 00 §10.4. Each generated file MUST carry the
//! configured SPDX identifier verbatim.

#[path = "common/mod.rs"]
mod common;

use fsm_codegen_c::{emit, CodegenConfig};

#[test]
fn default_license_is_mit_on_all_files() {
    let out = emit(&common::motor_ir(), &CodegenConfig::default()).unwrap();
    for f in &out.files {
        assert!(
            f.content.contains("SPDX-License-Identifier: MIT"),
            "{} missing MIT SPDX header",
            f.path
        );
    }
}

#[test]
fn apache_license_propagates_to_all_files() {
    let cfg = CodegenConfig {
        license_spdx: "Apache-2.0".into(),
        ..Default::default()
    };
    let out = emit(&common::motor_ir(), &cfg).unwrap();
    for f in &out.files {
        assert!(
            f.content.contains("SPDX-License-Identifier: Apache-2.0"),
            "{} missing Apache-2.0 SPDX header",
            f.path
        );
        // MIT must not survive into Apache output.
        assert!(
            !f.content.contains("SPDX-License-Identifier: MIT"),
            "{} still carries an MIT line",
            f.path
        );
    }
}

#[test]
fn arbitrary_spdx_identifier_passed_through_verbatim() {
    let cfg = CodegenConfig {
        license_spdx: "LicenseRef-CustomProprietary".into(),
        ..Default::default()
    };
    let out = emit(&common::motor_ir(), &cfg).unwrap();
    for f in &out.files {
        assert!(
            f.content
                .contains("SPDX-License-Identifier: LicenseRef-CustomProprietary"),
            "{} missing custom SPDX",
            f.path
        );
    }
}
