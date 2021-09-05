use syn::Path;

pub trait MatchesIdent {
    fn matches_ident(&self, ident: &str) -> bool;
}

impl MatchesIdent for Path {
    fn matches_ident(&self, target_id: &str) -> bool {
        self.get_ident()
            .map(|id| id.to_string() == target_id)
            .unwrap_or(false)
    }
}
