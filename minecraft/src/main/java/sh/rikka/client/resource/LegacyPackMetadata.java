package sh.rikka.client.resource;

import java.io.InputStream;
import net.minecraft.server.packs.AbstractPackMetadataResources;
import net.minecraft.server.packs.PackLocationInfo;
import net.minecraft.server.packs.resources.IoSupplier;
import org.jspecify.annotations.Nullable;

final class LegacyPackMetadata extends AbstractPackMetadataResources {
	private final LegacyPackRoot root;

	LegacyPackMetadata(PackLocationInfo location, LegacyPackRoot root) {
		super(location);
		this.root = root;
	}

	@Override
	public @Nullable IoSupplier<InputStream> getRootResource(String... path) {
		return this.root.resource(path);
	}

	@Override
	public void close() {
	}
}
