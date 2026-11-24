package sh.rikka.client.resource;

import java.nio.file.Path;
import java.util.stream.Stream;
import net.minecraft.server.packs.PackLocationInfo;
import net.minecraft.server.packs.PackMetadataResources;
import net.minecraft.server.packs.PackResources;
import net.minecraft.server.packs.repository.Pack;
import sh.rikka.client.resource.legacy.PackConverter;
import sh.rikka.client.resource.legacy.PackDescriptor;

final class LegacyPackSupplier implements Pack.ResourcesSupplier {
	private final LegacyPackRoot root;
	private final PackDescriptor descriptor;

	LegacyPackSupplier(Path content, PackDescriptor descriptor, int currentFormat) {
		this.root = new LegacyPackRoot(content, PackConverter.metadata(descriptor, currentFormat));
		this.descriptor = descriptor;
	}

	@Override
	public PackMetadataResources openMetadata(PackLocationInfo location) {
		return new LegacyPackMetadata(location, this.root);
	}

	@Override
	public Stream<PackResources> openResources(PackLocationInfo location, Pack.Metadata metadata) {
		return Stream.of(new LegacyPackResources(location, this.root, this.descriptor));
	}
}
