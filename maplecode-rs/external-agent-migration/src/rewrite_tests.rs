use super::*;
use pretty_assertions::assert_eq;

const PROFILE: RewriteProfile = RewriteProfile::new("SOURCE.md", &["source agent"])
    .with_case_sensitive_term_variants(&["Source"]);

#[test]
fn rewrites_terms_only_at_word_boundaries() {
    assert_eq!(
        PROFILE.rewrite("SOURCE.md Source source agent source_agent Source: café ‘Source’"),
        "AGENTS.md Maple Code Maple Code source_agent MapleCode: café ‘Maple Code’"
    );
}

#[test]
fn preserves_urls_and_emails_while_rewriting_surrounding_text() {
    assert_eq!(
        PROFILE.rewrite(
            r#"Source: https://Source.example/SOURCE.md?product=Source#Source
[Source](HTTPS://Source.example/a_(Source)/source.md "Source")Source
<http://Source.example/SOURCE.md>，Source
“https://Source.example/é/SOURCE.md” Source
`file:///Source/SOURCE.md` Source://Source/SOURCE.md
[Source](mailto:Source@example.com) [SOURCE.md](./SOURCE.md)
WWW.Source.example/SOURCE.md Source
https://Source.example/it's/SOURCE.md Source
<Source@example.com> Source@example.com support@Source.example Source
[Source](/Source/reference) [SOURCE.md](./Source/SOURCE.md)
SOURCE.md?section=details
[Source](#Source)
<a id="Source"></a>
![Source](https://example.com/Source/SOURCE.md "Source")
[![logo][SOURCE.md logo]](https://Source.example/Source)

[SOURCE.md logo]: ./logo.svg

| URL | Agent | Instructions |
| --- | --- | --- |
|https://example.com|Source|[docs](SOURCE.md)|

[Source][Source] [Source][] [Source]

[Source]: https://example.com/Source/SOURCE.md "Source"
"#,
        ),
        r#"MapleCode: https://Source.example/SOURCE.md?product=Source#Source
[Maple Code](HTTPS://Source.example/a_(Source)/source.md "Maple Code")Maple Code
<http://Source.example/SOURCE.md>，Maple Code
“https://Source.example/é/SOURCE.md” Maple Code
`file:///Source/SOURCE.md` Source://Source/SOURCE.md
[Maple Code](mailto:Source@example.com) [AGENTS.md](./AGENTS.md)
WWW.Source.example/SOURCE.md Maple Code
https://Source.example/it's/SOURCE.md Maple Code
<Source@example.com> Source@example.com support@Source.example Maple Code
[Maple Code](/MapleCode/reference) [AGENTS.md](./MapleCode/AGENTS.md)
AGENTS.md?section=details
[Maple Code](#Maple Code)
<a id="Maple Code"></a>
![Maple Code](https://example.com/Source/SOURCE.md "Maple Code")
[![logo][AGENTS.md logo]](https://Source.example/Source)

[AGENTS.md logo]: ./logo.svg

| URL | Agent | Instructions |
| --- | --- | --- |
|https://example.com|Codex|[docs](AGENTS.md)|

[Maple Code][Maple Code] [Maple Code][] [Maple Code]

[Maple Code]: https://example.com/Source/SOURCE.md "Maple Code"
"#
    );
}
