use pluto::parser::ast::*;
use pluto::span::Span;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclKind {
    Function,
    Class,
    Enum,
    EnumVariant,
    Trait,
    TraitMethod,
    Error,
    App,
    Field,
    Param,
}

/// Borrowed handle to a declaration in a Module.
pub struct DeclRef<'a> {
    kind: DeclKind,
    inner: DeclInner<'a>,
}

enum DeclInner<'a> {
    Function(&'a Function),
    Class(&'a ClassDecl),
    Enum(&'a EnumDecl),
    EnumVariant(&'a EnumVariant),
    Trait(&'a TraitDecl),
    TraitMethod(&'a TraitMethod),
    Error(&'a ErrorDecl),
    App(&'a AppDecl),
    Field(&'a Field),
    Param(&'a Param),
}

impl<'a> DeclRef<'a> {
    pub fn id(&self) -> Uuid {
        match &self.inner {
            DeclInner::Function(f) => f.id,
            DeclInner::Class(c) => c.id,
            DeclInner::Enum(e) => e.id,
            DeclInner::EnumVariant(v) => v.id,
            DeclInner::Trait(t) => t.id,
            DeclInner::TraitMethod(m) => m.id,
            DeclInner::Error(e) => e.id,
            DeclInner::App(a) => a.id,
            DeclInner::Field(f) => f.id,
            DeclInner::Param(p) => p.id,
        }
    }

    pub fn name(&self) -> &str {
        match &self.inner {
            DeclInner::Function(f) => &f.name.node,
            DeclInner::Class(c) => &c.name.node,
            DeclInner::Enum(e) => &e.name.node,
            DeclInner::EnumVariant(v) => &v.name.node,
            DeclInner::Trait(t) => &t.name.node,
            DeclInner::TraitMethod(m) => &m.name.node,
            DeclInner::Error(e) => &e.name.node,
            DeclInner::App(a) => &a.name.node,
            DeclInner::Field(f) => &f.name.node,
            DeclInner::Param(p) => &p.name.node,
        }
    }

    pub fn span(&self) -> Span {
        match &self.inner {
            DeclInner::Function(f) => f.name.span,
            DeclInner::Class(c) => c.name.span,
            DeclInner::Enum(e) => e.name.span,
            DeclInner::EnumVariant(v) => v.name.span,
            DeclInner::Trait(t) => t.name.span,
            DeclInner::TraitMethod(m) => m.name.span,
            DeclInner::Error(e) => e.name.span,
            DeclInner::App(a) => a.name.span,
            DeclInner::Field(f) => f.name.span,
            DeclInner::Param(p) => p.name.span,
        }
    }

    pub fn kind(&self) -> DeclKind {
        self.kind
    }

    pub fn as_function(&self) -> Option<&'a Function> {
        if let DeclInner::Function(f) = &self.inner { Some(f) } else { None }
    }

    pub fn as_class(&self) -> Option<&'a ClassDecl> {
        if let DeclInner::Class(c) = &self.inner { Some(c) } else { None }
    }

    pub fn as_enum(&self) -> Option<&'a EnumDecl> {
        if let DeclInner::Enum(e) = &self.inner { Some(e) } else { None }
    }

    pub fn as_enum_variant(&self) -> Option<&'a EnumVariant> {
        if let DeclInner::EnumVariant(v) = &self.inner { Some(v) } else { None }
    }

    pub fn as_trait(&self) -> Option<&'a TraitDecl> {
        if let DeclInner::Trait(t) = &self.inner { Some(t) } else { None }
    }

    pub fn as_trait_method(&self) -> Option<&'a TraitMethod> {
        if let DeclInner::TraitMethod(m) = &self.inner { Some(m) } else { None }
    }

    pub fn as_error(&self) -> Option<&'a ErrorDecl> {
        if let DeclInner::Error(e) = &self.inner { Some(e) } else { None }
    }

    pub fn as_app(&self) -> Option<&'a AppDecl> {
        if let DeclInner::App(a) = &self.inner { Some(a) } else { None }
    }

    pub fn as_field(&self) -> Option<&'a Field> {
        if let DeclInner::Field(f) = &self.inner { Some(f) } else { None }
    }

    pub fn as_param(&self) -> Option<&'a Param> {
        if let DeclInner::Param(p) = &self.inner { Some(p) } else { None }
    }

    // Construction helpers (pub(crate) so only module.rs / index.rs can create them)
    pub(crate) fn function(f: &'a Function) -> Self {
        Self { kind: DeclKind::Function, inner: DeclInner::Function(f) }
    }
    pub(crate) fn class(c: &'a ClassDecl) -> Self {
        Self { kind: DeclKind::Class, inner: DeclInner::Class(c) }
    }
    pub(crate) fn enum_decl(e: &'a EnumDecl) -> Self {
        Self { kind: DeclKind::Enum, inner: DeclInner::Enum(e) }
    }
    pub(crate) fn enum_variant(v: &'a EnumVariant) -> Self {
        Self { kind: DeclKind::EnumVariant, inner: DeclInner::EnumVariant(v) }
    }
    pub(crate) fn trait_decl(t: &'a TraitDecl) -> Self {
        Self { kind: DeclKind::Trait, inner: DeclInner::Trait(t) }
    }
    pub(crate) fn trait_method(m: &'a TraitMethod) -> Self {
        Self { kind: DeclKind::TraitMethod, inner: DeclInner::TraitMethod(m) }
    }
    pub(crate) fn error_decl(e: &'a ErrorDecl) -> Self {
        Self { kind: DeclKind::Error, inner: DeclInner::Error(e) }
    }
    pub(crate) fn app(a: &'a AppDecl) -> Self {
        Self { kind: DeclKind::App, inner: DeclInner::App(a) }
    }
    pub(crate) fn field(f: &'a Field) -> Self {
        Self { kind: DeclKind::Field, inner: DeclInner::Field(f) }
    }
    pub(crate) fn param(p: &'a Param) -> Self {
        Self { kind: DeclKind::Param, inner: DeclInner::Param(p) }
    }
}
