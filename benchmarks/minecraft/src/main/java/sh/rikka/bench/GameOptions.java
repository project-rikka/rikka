package sh.rikka.bench;

import java.io.IOException;
import java.io.UncheckedIOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.stream.Collectors;
import net.minecraft.SharedConstants;

final class GameOptions {
	private static final String FILE = "options.txt";
	private static final int UNLIMITED_FRAMERATE = 260;

	private GameOptions() {
	}

	static void write(Path gameDirectory, Preset preset) {
		Map<String, Object> options = new LinkedHashMap<>();
		options.put("version", SharedConstants.getCurrentVersion().dataVersion().version());
		options.put("preferredGraphicsBackend", "\"vulkan\"");
		options.put("graphicsPreset", "\"custom\"");
		options.put("enableVsync", false);
		options.put("maxFps", UNLIMITED_FRAMERATE);
		options.put("inactivityFpsLimit", "\"minimized\"");
		options.put("pauseOnLostFocus", false);
		options.put("fullscreen", false);
		options.put("guiScale", 2);
		options.put("fov", 0.0);
		options.put("fovEffectScale", 1.0);
		options.put("gamma", 0.5);
		options.put("bobView", true);
		options.put("vignette", true);
		options.put("chunkSectionFadeInTime", 0.75);
		options.put("glintSpeed", 0.0);
		options.put("renderDistance", preset.renderDistance);
		options.put("simulationDistance", preset.simulationDistance);
		options.put("biomeBlendRadius", preset.biomeBlendRadius);
		options.put("prioritizeChunkUpdates", preset.prioritizeChunkUpdates);
		options.put("ao", preset.ambientOcclusion);
		options.put("renderClouds", '"' + preset.clouds + '"');
		options.put("particles", preset.particles);
		options.put("mipmapLevels", preset.mipmapLevels);
		options.put("entityShadows", preset.entityShadows);
		options.put("entityDistanceScaling", preset.entityDistanceScaling);
		options.put("cloudRange", preset.cloudRange);
		options.put("cutoutLeaves", preset.cutoutLeaves);
		options.put("improvedTransparency", preset.improvedTransparency);
		options.put("weatherRadius", preset.weatherRadius);
		options.put("maxAnisotropyBit", preset.maxAnisotropyBit);
		options.put("textureFiltering", preset.textureFiltering);
		options.put("narrator", 0);
		options.put("tutorialStep", "none");
		options.put("onboardAccessibility", false);
		options.put("skipMultiplayerWarning", true);
		options.put("joinedFirstServer", true);
		options.put("realmsNotifications", false);
		options.put("soundCategory_master", 0.0);
		options.put("lang", "en_us");
		String text = options.entrySet().stream().map(option -> option.getKey() + ":" + option.getValue()).collect(Collectors.joining("\n", "", "\n"));
		try {
			Files.createDirectories(gameDirectory);
			Files.writeString(gameDirectory.resolve(FILE), text);
		} catch (IOException error) {
			throw new UncheckedIOException("failed to write " + FILE, error);
		}
	}
}
