//! Typed AST accessors for top-level declarations.

use crate::cst::SyntaxKind;

use super::{ast_node, children, first_ident, AstChildren};

ast_node!(File, FILE);
ast_node!(LanguageDecl, LANGUAGE_DECL);
ast_node!(LanguageVersion, LANGUAGE_VERSION);
ast_node!(ImportDecl, IMPORT_DECL);
ast_node!(ImportAlias, IMPORT_ALIAS);
ast_node!(ImportList, IMPORT_LIST);
ast_node!(ImportItem, IMPORT_ITEM);
ast_node!(FeatureDecl, FEATURE_DECL);
ast_node!(ConstDecl, CONST_DECL);
ast_node!(ConstExpr, CONST_EXPR);
ast_node!(EnumDecl, ENUM_DECL);
ast_node!(EnumVariant, ENUM_VARIANT);
ast_node!(ExternDecl, EXTERN_DECL);
ast_node!(ParamList, PARAM_LIST);
ast_node!(Param, PARAM);
ast_node!(MachineDecl, MACHINE_DECL);
ast_node!(ContextBlock, CONTEXT_BLOCK);
ast_node!(FieldDecl, FIELD_DECL);
ast_node!(EventsBlock, EVENTS_BLOCK);
ast_node!(EventDecl, EVENT_DECL);
ast_node!(PayloadList, PAYLOAD_LIST);
ast_node!(PayloadField, PAYLOAD_FIELD);
ast_node!(QueueBlock, QUEUE_BLOCK);
ast_node!(TargetBlock, TARGET_BLOCK);
ast_node!(ConfigEntry, CONFIG_ENTRY);
ast_node!(InitialDecl, INITIAL_DECL);
ast_node!(StableIdAnnot, STABLE_ID_ANNOT);
ast_node!(TypeRef, TYPE_REF);
ast_node!(OpaqueTypeRef, OPAQUE_TYPE_REF);

impl File {
    pub fn language_decl(&self) -> Option<LanguageDecl> {
        super::child(&self.0)
    }
    pub fn imports(&self) -> AstChildren<ImportDecl> {
        children(&self.0)
    }
    pub fn features(&self) -> AstChildren<FeatureDecl> {
        children(&self.0)
    }
    pub fn consts(&self) -> AstChildren<ConstDecl> {
        children(&self.0)
    }
    pub fn enums(&self) -> AstChildren<EnumDecl> {
        children(&self.0)
    }
    pub fn externs(&self) -> AstChildren<ExternDecl> {
        children(&self.0)
    }
    pub fn machines(&self) -> AstChildren<MachineDecl> {
        children(&self.0)
    }
}

impl LanguageDecl {
    pub fn version(&self) -> Option<LanguageVersion> {
        super::child(&self.0)
    }
}

impl ImportDecl {
    /// The string literal path token's text, with quotes stripped.
    pub fn path(&self) -> Option<String> {
        // The first `StringLiteral` token under us.
        let token = self
            .0
            .children_with_tokens()
            .filter_map(|el| el.into_token())
            .find(|t| t.kind() == SyntaxKind::StringLiteral)?;
        let text = token.text();
        Some(strip_quotes(text).to_string())
    }
    pub fn alias(&self) -> Option<ImportAlias> {
        super::child(&self.0)
    }
    pub fn items(&self) -> Option<ImportList> {
        super::child(&self.0)
    }
}

impl ImportAlias {
    pub fn name(&self) -> Option<String> {
        first_ident(&self.0)
    }
}

impl ImportList {
    pub fn items(&self) -> AstChildren<ImportItem> {
        children(&self.0)
    }
}

impl ImportItem {
    pub fn name(&self) -> Option<String> {
        first_ident(&self.0)
    }
}

impl FeatureDecl {
    pub fn name(&self) -> Option<String> {
        first_ident(&self.0)
    }
}

impl ConstDecl {
    pub fn name(&self) -> Option<String> {
        first_ident(&self.0)
    }
    pub fn value(&self) -> Option<ConstExpr> {
        super::child(&self.0)
    }
}

impl EnumDecl {
    pub fn name(&self) -> Option<String> {
        first_ident(&self.0)
    }
    pub fn variants(&self) -> AstChildren<EnumVariant> {
        children(&self.0)
    }
}

impl EnumVariant {
    pub fn name(&self) -> Option<String> {
        first_ident(&self.0)
    }
}

impl ExternDecl {
    /// `true` if the declaration begins with the `pure` keyword.
    pub fn is_pure(&self) -> bool {
        self.0
            .children_with_tokens()
            .filter_map(|el| el.into_token())
            .any(|t| t.kind() == SyntaxKind::KwPure)
    }
    pub fn name(&self) -> Option<String> {
        first_ident(&self.0)
    }
    pub fn params(&self) -> Option<ParamList> {
        super::child(&self.0)
    }
    pub fn return_type(&self) -> Option<TypeRef> {
        super::child(&self.0)
    }
}

impl ParamList {
    pub fn params(&self) -> AstChildren<Param> {
        children(&self.0)
    }
}

impl Param {
    pub fn name(&self) -> Option<String> {
        first_ident(&self.0)
    }
    pub fn ty(&self) -> Option<TypeRef> {
        super::child(&self.0)
    }
}

impl MachineDecl {
    /// `true` if the machine is `export`-marked.
    pub fn is_export(&self) -> bool {
        self.0
            .children_with_tokens()
            .filter_map(|el| el.into_token())
            .any(|t| t.kind() == SyntaxKind::KwExport)
    }
    pub fn name(&self) -> Option<String> {
        first_ident(&self.0)
    }
    pub fn stable_id(&self) -> Option<StableIdAnnot> {
        // The machine's stable-id is one of *its* CST children — the
        // grammar wraps `@id(...)` before the `machine` keyword inside the
        // MACHINE_DECL node, so a simple `child::<StableIdAnnot>` works.
        super::child(&self.0)
    }
    pub fn context(&self) -> Option<ContextBlock> {
        super::child(&self.0)
    }
    pub fn events(&self) -> Option<EventsBlock> {
        super::child(&self.0)
    }
    pub fn queue(&self) -> Option<QueueBlock> {
        super::child(&self.0)
    }
    pub fn target(&self) -> Option<TargetBlock> {
        super::child(&self.0)
    }
    pub fn initial(&self) -> Option<InitialDecl> {
        super::child(&self.0)
    }
    pub fn states(&self) -> AstChildren<crate::ast::StateDecl> {
        children(&self.0)
    }
    pub fn externs(&self) -> AstChildren<ExternDecl> {
        children(&self.0)
    }
    pub fn regions(&self) -> AstChildren<crate::ast::RegionDecl> {
        children(&self.0)
    }
}

impl ContextBlock {
    pub fn fields(&self) -> AstChildren<FieldDecl> {
        children(&self.0)
    }
}

impl FieldDecl {
    pub fn name(&self) -> Option<String> {
        first_ident(&self.0)
    }
    pub fn ty(&self) -> Option<TypeRef> {
        super::child(&self.0)
    }
    pub fn default(&self) -> Option<ConstExpr> {
        super::child(&self.0)
    }
}

impl EventsBlock {
    pub fn events(&self) -> AstChildren<EventDecl> {
        children(&self.0)
    }
}

impl EventDecl {
    pub fn name(&self) -> Option<String> {
        first_ident(&self.0)
    }
    pub fn payload(&self) -> Option<PayloadList> {
        super::child(&self.0)
    }
}

impl PayloadList {
    pub fn fields(&self) -> AstChildren<PayloadField> {
        children(&self.0)
    }
}

impl PayloadField {
    pub fn name(&self) -> Option<String> {
        first_ident(&self.0)
    }
    pub fn ty(&self) -> Option<TypeRef> {
        super::child(&self.0)
    }
}

impl QueueBlock {
    pub fn entries(&self) -> AstChildren<ConfigEntry> {
        children(&self.0)
    }
}

impl TargetBlock {
    pub fn name(&self) -> Option<String> {
        first_ident(&self.0)
    }
    pub fn entries(&self) -> AstChildren<ConfigEntry> {
        children(&self.0)
    }
}

impl ConfigEntry {
    pub fn key(&self) -> Option<String> {
        first_ident(&self.0)
    }
}

impl InitialDecl {
    pub fn target(&self) -> Option<String> {
        first_ident(&self.0)
    }
}

impl StableIdAnnot {
    /// The raw stable-ID payload, with surrounding quotes (if any) stripped.
    pub fn id(&self) -> Option<String> {
        // For the `@some_id` short form the StableId token carries the
        // text. For the `@id("…")` long form the payload is the string
        // literal.
        for el in self.0.children_with_tokens() {
            if let Some(t) = el.as_token() {
                let kind = t.kind();
                if kind == SyntaxKind::StringLiteral {
                    return Some(strip_quotes(t.text()).to_string());
                }
                if kind == SyntaxKind::StableId {
                    // Strip the leading `@`.
                    return Some(t.text().trim_start_matches('@').to_string());
                }
            }
        }
        None
    }
}

fn strip_quotes(s: &str) -> &str {
    let s = s.strip_prefix('"').unwrap_or(s);
    s.strip_suffix('"').unwrap_or(s)
}
