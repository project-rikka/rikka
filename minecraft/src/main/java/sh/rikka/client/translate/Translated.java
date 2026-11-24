package sh.rikka.client.translate;

import java.util.Optional;

public record Translated(String html, Optional<String> detected) {
}
