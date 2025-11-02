/// Normalizes a path by removing trailing slashes (except for root "/")
pub fn normalize_path(path: &str) -> &str {
    if path.len() > 1 && path.ends_with('/') {
        &path[..path.len() - 1]
    } else {
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path() {
        // Root path should remain unchanged
        assert_eq!(normalize_path("/"), "/");

        // Paths with trailing slashes should have them removed
        assert_eq!(normalize_path("/apples/"), "/apples");
        assert_eq!(normalize_path("/a/b/c/"), "/a/b/c");
        assert_eq!(normalize_path("/foo/bar/"), "/foo/bar");
        assert_eq!(normalize_path("/test.html/"), "/test.html");

        // Paths without trailing slashes should remain unchanged
        assert_eq!(normalize_path("/apples"), "/apples");
        assert_eq!(normalize_path("/a/b/c"), "/a/b/c");
        assert_eq!(normalize_path("/test.html"), "/test.html");

        // Edge cases
        assert_eq!(normalize_path("/a/"), "/a");
        assert_eq!(normalize_path("/a"), "/a");
    }
}
