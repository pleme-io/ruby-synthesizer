/// INVARIANT: No Raw node construction in production code.

#[test]
fn no_raw_in_production_code() {
    let sources = [
        ("node.rs", include_str!("../src/node.rs")),
        ("emitter.rs", include_str!("../src/emitter.rs")),
        ("builders.rs", include_str!("../src/builders.rs")),
        ("rspec.rs", include_str!("../src/rspec.rs")),
    ];

    for (name, source) in &sources {
        let production_code = source.split("#[cfg(test)]").next().unwrap_or(source);
        let raw_constructors = production_code
            .lines()
            .filter(|line| {
                let trimmed = line.trim();
                !trimmed.starts_with("///")
                    && !trimmed.starts_with("//")
                    && !trimmed.starts_with("#[deprecated")
                    && !trimmed.starts_with("#[allow")
                    && !trimmed.starts_with("Self::Raw")
                    && !trimmed.contains("Raw(String)")
                    && trimmed.contains("RubyNode::Raw(")
            })
            .count();

        assert_eq!(
            raw_constructors, 0,
            "INVARIANT VIOLATION: {name} has {raw_constructors} Raw constructor(s)"
        );
    }
}
