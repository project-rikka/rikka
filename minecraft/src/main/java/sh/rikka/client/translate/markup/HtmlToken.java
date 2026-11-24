package sh.rikka.client.translate.markup;

import java.util.Map;
import java.util.Optional;

sealed interface HtmlToken {
	record Text(String text) implements HtmlToken {
	}

	record Open(String name, Map<String, String> attributes, boolean selfClosing) implements HtmlToken {
		Optional<String> attribute(String key) {
			return Optional.ofNullable(this.attributes.get(key));
		}
	}

	record Close(String name) implements HtmlToken {
	}
}
