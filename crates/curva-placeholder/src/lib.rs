//! Placeholder crate. It exists so the workspace, its rules and the automated
//! checks can be proven before any real Curva code is written.

/// Returns the name of the project.
pub fn project_name() -> &'static str {
    "curva"
}

#[cfg(test)]
mod tests {
    use super::project_name;

    #[test]
    fn names_the_project() {
        assert_eq!(project_name(), "curva");
    }
}
