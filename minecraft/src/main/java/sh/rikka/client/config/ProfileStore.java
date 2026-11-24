package sh.rikka.client.config;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonObject;
import com.google.gson.JsonParseException;
import com.google.gson.JsonParser;
import com.google.gson.JsonPrimitive;
import java.io.IOException;
import java.nio.channels.FileChannel;
import java.nio.charset.CharacterCodingException;
import java.nio.file.Files;
import java.nio.file.NoSuchFileException;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.nio.file.StandardOpenOption;
import java.util.List;
import java.util.Optional;
import java.util.regex.Pattern;
import java.util.stream.Stream;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

final class ProfileStore {
	private static final Logger LOGGER = LoggerFactory.getLogger(ProfileStore.class);
	private static final Gson GSON = new GsonBuilder().setPrettyPrinting().disableHtmlEscaping().create();
	private static final String EXTENSION = ".json";
	private static final String PROFILES = "profiles";
	private static final String STATE = "client.json";
	private static final String ACTIVE_PROFILE = "profile";
	private static final String PREFERENCES = "preferences";
	private static final Pattern NAME = Pattern.compile("[\\p{L}\\p{N}_-]+( [\\p{L}\\p{N}_-]+)*");

	private final Path profiles;
	private final Path state;

	ProfileStore(Path directory) {
		this.profiles = directory.resolve(PROFILES);
		this.state = directory.resolve(STATE);
	}

	static boolean isValidName(String name) {
		return name.length() <= ProfileManager.MAX_NAME_LENGTH && NAME.matcher(name).matches();
	}

	List<String> list() throws IOException {
		try (Stream<Path> files = Files.list(this.profiles)) {
			return files
				.filter(Files::isRegularFile)
				.map(file -> file.getFileName().toString())
				.filter(name -> name.endsWith(EXTENSION))
				.map(name -> name.substring(0, name.length() - EXTENSION.length()))
				.filter(ProfileStore::isValidName)
				.sorted()
				.toList();
		} catch (NoSuchFileException failure) {
			return List.of();
		}
	}

	boolean exists(String name) {
		return Files.isRegularFile(file(name));
	}

	JsonObject read(String name) throws IOException {
		return readObject(file(name)).orElseGet(JsonObject::new);
	}

	void write(String name, JsonObject document) throws IOException {
		writeAtomically(file(name), GSON.toJson(document));
	}

	void delete(String name) throws IOException {
		Files.delete(file(name));
	}

	Optional<String> readActive() throws IOException {
		return readObject(this.state).map(document -> document.get(ACTIVE_PROFILE)).flatMap(element ->
			element instanceof JsonPrimitive primitive && primitive.isString() ? Optional.of(primitive.getAsString()) : Optional.empty()
		);
	}

	JsonObject readPreferences() throws IOException {
		return readObject(this.state)
			.map(document -> document.get(PREFERENCES) instanceof JsonObject preferences ? preferences : new JsonObject())
			.orElseGet(JsonObject::new);
	}

	void writeState(String active, JsonObject preferences) throws IOException {
		JsonObject document = readObject(this.state).orElseGet(JsonObject::new);
		document.addProperty(ACTIVE_PROFILE, active);
		document.add(PREFERENCES, preferences.deepCopy());
		writeAtomically(this.state, GSON.toJson(document));
	}

	private Path file(String name) {
		return this.profiles.resolve(name + EXTENSION);
	}

	private static Optional<JsonObject> readObject(Path file) throws IOException {
		try {
			if (JsonParser.parseString(Files.readString(file)) instanceof JsonObject object) {
				return Optional.of(object);
			}
			backUp(file, "expected a JSON object");
		} catch (NoSuchFileException failure) {
			return Optional.empty();
		} catch (CharacterCodingException | JsonParseException failure) {
			backUp(file, failure.toString());
		}
		return Optional.empty();
	}

	private static void backUp(Path file, String reason) throws IOException {
		Path backup = file.resolveSibling(file.getFileName() + "." + System.currentTimeMillis() + ".corrupt");
		Files.move(file, backup);
		LOGGER.warn("moved unreadable {} to {}: {}", file, backup, reason);
	}

	private static void writeAtomically(Path target, String text) throws IOException {
		Path directory = target.getParent();
		Files.createDirectories(directory);
		Path temporary = Files.createTempFile(directory, target.getFileName().toString(), ".tmp");
		try {
			Files.writeString(temporary, text);
			try (FileChannel channel = FileChannel.open(temporary, StandardOpenOption.WRITE)) {
				channel.force(true);
			}
			Files.move(temporary, target, StandardCopyOption.ATOMIC_MOVE, StandardCopyOption.REPLACE_EXISTING);
		} finally {
			Files.deleteIfExists(temporary);
		}
	}
}
