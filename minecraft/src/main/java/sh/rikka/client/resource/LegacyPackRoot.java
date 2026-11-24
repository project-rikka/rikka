package sh.rikka.client.resource;

import java.io.ByteArrayInputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Path;
import java.util.Optional;
import net.minecraft.server.packs.PackResources;
import net.minecraft.server.packs.resources.IoSupplier;
import org.jspecify.annotations.Nullable;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import sh.rikka.client.resource.legacy.PackArchive;

record LegacyPackRoot(Path content, byte[] metadata) {
	private static final Logger LOGGER = LoggerFactory.getLogger(LegacyPackRoot.class);

	@Nullable IoSupplier<InputStream> resource(String... path) {
		String name = String.join("/", path);
		if (name.equals(PackResources.PACK_META)) {
			return () -> new ByteArrayInputStream(this.metadata);
		}
		Optional<byte[]> bytes;
		try {
			bytes = PackArchive.readRoot(this.content, name);
		} catch (IOException failure) {
			LOGGER.warn("failed to read {} from legacy pack {}: {}", name, this.content, failure.getMessage());
			return null;
		}
		return bytes.<IoSupplier<InputStream>>map(data -> () -> new ByteArrayInputStream(data)).orElse(null);
	}
}
