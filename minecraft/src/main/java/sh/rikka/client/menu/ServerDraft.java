package sh.rikka.client.menu;

import java.util.Optional;
import os.rikka.TextRole;
import os.rikka.TextTone;
import os.rikka.View;
import sh.rikka.client.config.ProfileManager;
import sh.rikka.client.config.ServerPattern;

sealed interface ServerDraft {
	static ServerDraft of(ProfileManager profiles, String text) {
		String address = text.strip();
		if (address.isEmpty()) {
			return new Blank();
		}
		Optional<ServerPattern> parsed = ServerPattern.parse(address);
		if (parsed.isEmpty()) {
			return new Rejected(Labels.of("profiles.server.invalid"));
		}
		ServerPattern pattern = parsed.get();
		return profiles.serverOwner(pattern)
			.<ServerDraft>map(owner -> new Rejected(Labels.of("profiles.server.taken", owner)))
			.orElseGet(() -> new Ready(pattern));
	}

	default void viewProblem(View view) {
		if (this instanceof Rejected rejected) {
			view.text(rejected.problem()).textStyle(TextRole.CAPTION).tone(TextTone.DANGER);
		}
	}

	record Blank() implements ServerDraft {
	}

	record Rejected(String problem) implements ServerDraft {
	}

	record Ready(ServerPattern pattern) implements ServerDraft {
	}
}
