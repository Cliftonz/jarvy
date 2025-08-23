use std::fs;

#[test]
fn docs_error_codes_markdown_contains_all_codes() {
    // Load the markdown file
    let md = fs::read_to_string("docs/error-codes.md").expect("docs/error-codes.md should exist");

    // Obtain the canonical list from code
    let codes = crate::error_codes::list_error_codes();

    for info in codes {
        // Check for anchor presence
        let anchor = format!("<a id=\"{}\"></a>", info.slug);
        assert!(
            md.contains(&anchor),
            "Missing anchor for slug: {}",
            info.slug
        );

        // Check that key and code appear in the document
        assert!(md.contains(info.key), "Missing key in docs: {}", info.key);
        assert!(
            md.contains(&info.code.to_string()),
            "Missing code number in docs: {}",
            info.code
        );
    }
}
