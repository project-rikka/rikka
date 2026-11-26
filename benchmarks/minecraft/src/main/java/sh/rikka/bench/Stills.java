package sh.rikka.bench;

import com.google.gson.JsonObject;
import com.mojang.blaze3d.platform.NativeImage;
import java.io.IOException;
import java.io.InputStream;
import java.io.UncheckedIOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Optional;
import java.util.concurrent.CompletableFuture;
import net.minecraft.client.Minecraft;
import net.minecraft.client.Screenshot;
import net.minecraft.util.ARGB;

final class Stills {
	private static final String EXTENSION = ".png";
	private static final String DIFFERENCE_SUFFIX = ".difference.png";
	private static final int SAME = 0xFF000000;
	private static final int DIFFERENT = 0xFFFFFFFF;

	private final Path output;
	private final Optional<Path> reference;

	Stills(Path output, Optional<Path> reference) {
		this.output = output;
		this.reference = reference;
	}

	CompletableFuture<JsonObject> capture(Minecraft minecraft, String name) {
		CompletableFuture<JsonObject> result = new CompletableFuture<>();
		Screenshot.takeScreenshot(minecraft.gameRenderer.mainRenderTarget(), image -> {
			try (image) {
				result.complete(this.save(name, image));
			} catch (IOException | RuntimeException error) {
				result.completeExceptionally(error);
			}
		});
		return result;
	}

	private JsonObject save(String name, NativeImage image) throws IOException {
		image.writeToFile(this.output.resolve(name + EXTENSION));
		JsonObject result = new JsonObject();
		result.addProperty("width", image.getWidth());
		result.addProperty("height", image.getHeight());
		if (this.reference.isPresent()) {
			result.add("difference", this.compare(name, image, this.reference.get().resolve(name + EXTENSION)));
		}
		return result;
	}

	private JsonObject compare(String name, NativeImage image, Path referenceFile) throws IOException {
		try (InputStream stream = Files.newInputStream(referenceFile); NativeImage expected = NativeImage.read(stream)) {
			if (expected.getWidth() != image.getWidth() || expected.getHeight() != image.getHeight()) {
				throw new UncheckedIOException(new IOException(referenceFile + " has another size"));
			}
			try (NativeImage mask = new NativeImage(image.getWidth(), image.getHeight(), false)) {
				long differing = 0;
				long sum = 0;
				int largest = 0;
				for (int y = 0; y < image.getHeight(); y++) {
					for (int x = 0; x < image.getWidth(); x++) {
						int distance = distance(image.getPixel(x, y), expected.getPixel(x, y));
						mask.setPixel(x, y, distance == 0 ? SAME : DIFFERENT);
						differing += distance == 0 ? 0 : 1;
						sum += distance;
						largest = Math.max(largest, distance);
					}
				}
				long pixels = (long) image.getWidth() * image.getHeight();
				if (differing > 0) {
					mask.writeToFile(this.output.resolve(name + DIFFERENCE_SUFFIX));
				}
				JsonObject difference = new JsonObject();
				difference.addProperty("differing_pixels", differing);
				difference.addProperty("differing_fraction", differing / (double) pixels);
				difference.addProperty("largest_channel_difference", largest);
				difference.addProperty("mean_channel_difference", sum / (double) pixels);
				return difference;
			}
		}
	}

	private static int distance(int actual, int expected) {
		int red = Math.abs(ARGB.red(actual) - ARGB.red(expected));
		int green = Math.abs(ARGB.green(actual) - ARGB.green(expected));
		int blue = Math.abs(ARGB.blue(actual) - ARGB.blue(expected));
		return Math.max(red, Math.max(green, blue));
	}
}
