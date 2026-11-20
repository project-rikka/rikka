use rikka_html::{Escaped, decode};

#[test]
fn character_references_decode() {
    let cases = [
        ("plain", "plain"),
        (
            "&lt;a&gt; &amp; &quot;b&quot; &apos;c&#39;",
            "<a> & \"b\" 'c'",
        ),
        (
            "&nbsp;&copy;&eacute;&Omega;&omega;&hellip;&rarr;",
            "\u{a0}©éΩω…→",
        ),
        ("&#x41;&#66;&#X43", "ABC"),
        ("&#150;&#0;&#xD800;&#x110000;", "–\u{fffd}\u{fffd}\u{fffd}"),
        ("&unknown; &amp &; &#; a&b=c", "&unknown; &amp &; &#; a&b=c"),
    ];
    for (encoded, expected) in cases {
        assert_eq!(decode(encoded), expected, "{encoded}");
    }
}

#[test]
fn escaped_text_survives_decoding() {
    let hostile = "<script>alert(\"1\" + '2')</script> & more";
    let escaped = Escaped(hostile).to_string();
    assert_eq!(
        escaped,
        "&lt;script&gt;alert(&quot;1&quot; + &#39;2&#39;)&lt;/script&gt; &amp; more"
    );
    assert_eq!(decode(&escaped), hostile);
}
