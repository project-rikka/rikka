package sh.rikka.bench;

import java.nio.file.Path;
import java.util.Locale;
import java.util.Optional;
import java.util.Set;

public record Settings(String label, Preset preset, Path output, Optional<Path> reference, Optional<Set<String>> scenes) {
	private static final String LABEL = "RIKKA_BENCH_LABEL";
	private static final String PRESET = "RIKKA_BENCH_PRESET";
	private static final String REFERENCE = "RIKKA_BENCH_REFERENCE";
	private static final String SCENES = "RIKKA_BENCH_SCENES";
	private static final String RESULTS = "bench";

	public static Settings fromEnvironment(Path gameDirectory) {
		String label = require(LABEL);
		Preset preset = Preset.valueOf(Optional.ofNullable(System.getenv(PRESET)).orElse("fancy").toUpperCase(Locale.ROOT));
		Path results = gameDirectory.resolve(RESULTS);
		Optional<Path> reference = Optional.ofNullable(System.getenv(REFERENCE)).map(results::resolve);
		Optional<Set<String>> scenes = Optional.ofNullable(System.getenv(SCENES)).map(names -> Set.of(names.split(",")));
		return new Settings(label, preset, results.resolve(label), reference, scenes);
	}

	public boolean includes(String scene) {
		return this.scenes.isEmpty() || this.scenes.get().contains(scene);
	}

	private static String require(String variable) {
		String value = System.getenv(variable);
		if (value == null || value.isBlank()) {
			throw new IllegalStateException(variable + " is not set");
		}
		return value;
	}
}
