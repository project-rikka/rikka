package sh.rikka.client.resource;

import java.io.IOException;
import java.nio.file.Path;
import java.util.Optional;
import net.minecraft.SharedConstants;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.repository.Pack;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import sh.rikka.client.resource.legacy.PackDescriptor;

public final class LegacyPacks {
	private static final Logger LOGGER = LoggerFactory.getLogger(LegacyPacks.class);

	private LegacyPacks() {
	}

	public static Pack.ResourcesSupplier supplier(Path content, Pack.ResourcesSupplier original) {
		Optional<PackDescriptor> descriptor;
		try {
			descriptor = PackDescriptor.read(content);
		} catch (IOException | RuntimeException failure) {
			LOGGER.debug("left pack {} unconverted: {}", content, failure.getMessage());
			return original;
		}
		int current = SharedConstants.getCurrentVersion().packVersion(PackType.CLIENT_RESOURCES).major();
		if (descriptor.isEmpty() || descriptor.get().format() >= current) {
			return original;
		}
		return new LegacyPackSupplier(content, descriptor.get(), current);
	}
}
