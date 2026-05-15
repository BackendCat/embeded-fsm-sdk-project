//! License-header test — Doc 00 §10.4. Each generated file MUST carry the
//! configured SPDX identifier verbatim.
//!
//! ## Test classification (v1.1-W0 / SUBAGENT_CONVENTIONS §5.4, PD-2)
//!
//! These `.contains()` are a **legitimate structural invariant**, not a
//! P0-1-class behaviour proxy. "Every emitted file carries the exact SPDX
//! string `SPDX-License-Identifier: <id>` and the previous license does
//! NOT survive a license change" is a verbatim file-content contract
//! (Doc 00 §10.4). It has no executable behaviour to observe — a license
//! header is a comment; asserting its presence/absence in the generated
//! text IS the correct and only meaningful check. Not converted.

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
