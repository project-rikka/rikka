package sh.rikka.client.config;

import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.SortedMap;
import java.util.TreeMap;

final class ServerIndex {
	private final SortedMap<String, List<ServerPattern>> servers = new TreeMap<>();

	List<ServerPattern> get(String profile) {
		return this.servers.getOrDefault(profile, List.of());
	}

	void put(String profile, List<ServerPattern> patterns) {
		if (patterns.isEmpty()) {
			this.servers.remove(profile);
		} else {
			this.servers.put(profile, List.copyOf(patterns));
		}
	}

	void remove(String profile) {
		this.servers.remove(profile);
	}

	Optional<String> owner(ServerPattern pattern) {
		return this.servers.entrySet().stream().filter(entry -> entry.getValue().contains(pattern)).map(Map.Entry::getKey).findFirst();
	}

	Optional<String> match(String host, int port) {
		return ServerPattern.Host.of(host).flatMap(candidate -> this.servers.entrySet()
			.stream()
			.flatMap(entry -> entry.getValue()
				.stream()
				.filter(pattern -> pattern.matches(candidate, port))
				.map(pattern -> new Match(entry.getKey(), pattern)))
			.reduce((best, next) -> ServerPattern.SPECIFICITY.compare(next.pattern(), best.pattern()) > 0 ? next : best)
			.map(Match::profile));
	}

	private record Match(String profile, ServerPattern pattern) {
	}
}
