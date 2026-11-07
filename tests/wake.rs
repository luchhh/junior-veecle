use junior_veecle::wake::is_wake_phrase;

#[test]
fn wakes_on_real_transcripts() {
    // What the on-device model returned for the wake phrases, recorded on the robot.
    for text in [
        "Junior, despierta.",
        "Despierta, Junior.",
        "O Junior, despierta ya.",
        "Hoy Junior despierta ya.",
        "Junior, wake up.",
        "despierta Junior.",
        // Variants it may produce.
        "¡Júnior! ¡Despiértate!",
        "Yunior, despierta",
        "junior desperta",
        "JUNIOR, WAKE UP!",
    ] {
        assert!(is_wake_phrase(text), "should wake: {text:?}");
    }
}

#[test]
fn stays_asleep_otherwise() {
    for text in [
        // Other sentences from the same recording.
        "Junior es un robot muy lindo.",
        "Hoy me esperaste de imprando.",
        "Hoy me esperaste de temprano.",
        "¿Donde hay que eliminar esta azul?",
        "¿Dónde dejé mi maleta azul?",
        "Despacio por favor.",
        // A wake word without the name, or the reverse.
        "Despierta, que ya es tarde.",
        "wake up",
        "Junior, ¿estás ahí?",
        "",
    ] {
        assert!(!is_wake_phrase(text), "should stay asleep: {text:?}");
    }
}
