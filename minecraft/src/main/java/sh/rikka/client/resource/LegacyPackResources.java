package sh.rikka.client.resource;

import java.io.ByteArrayInputStream;
import java.io.IOException;
import java.io.InputStream;
import java.util.Optional;
import java.util.Set;
import net.minecraft.resources.Identifier;
import net.minecraft.server.packs.AbstractPackMetadataResources;
import net.minecraft.server.packs.PackLocationInfo;
import net.minecraft.server.packs.PackResources;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.resources.IoSupplier;
import org.jspecify.annotations.Nullable;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import sh.rikka.client.resource.legacy.Asset;
import sh.rikka.client.resource.legacy.Assets;
import sh.rikka.client.resource.legacy.PackArchive;
import sh.rikka.client.resource.legacy.PackConverter;
import sh.rikka.client.resource.legacy.PackDescriptor;
import sh.rikka.client.resource.legacy.Vanilla;

final class LegacyPackResources extends AbstractPackMetadataResources implements PackResources {
	private static final Logger LOGGER = LoggerFactory.getLogger(LegacyPackResources.class);

	private final LegacyPackRoot root;
	private final PackDescriptor descriptor;
	private final Object lock = new Object();
	private boolean opened;
	private @Nullable PackArchive archive;
	private @Nullable Assets assets;

	LegacyPackResources(PackLocationInfo location, LegacyPackRoot root, PackDescriptor descriptor) {
		super(location);
		this.root = root;
		this.descriptor = descriptor;
	}

	@Override
	public @Nullable IoSupplier<InputStream> getRootResource(String... path) {
		return this.root.resource(path);
	}

	@Override
	public @Nullable IoSupplier<InputStream> getResource(PackType type, Identifier location) {
		if (type != PackType.CLIENT_RESOURCES) {
			return null;
		}
		Optional<Asset> asset = assets().get(Assets.ROOT + location.getNamespace() + "/" + location.getPath());
		return asset.map(LegacyPackResources::supplier).orElse(null);
	}

	@Override
	public void listResources(PackType type, String namespace, String directory, PackResources.ResourceOutput output) {
		if (type != PackType.CLIENT_RESOURCES) {
			return;
		}
		Assets converted = assets();
		String namespaceRoot = Assets.ROOT + namespace + "/";
		for (String path : converted.paths(namespaceRoot + directory + "/")) {
			Identifier id = Identifier.tryBuild(namespace, path.substring(namespaceRoot.length()));
			if (id == null) {
				LOGGER.warn("invalid path in legacy pack {}: {}, ignoring", packId(), path);
				continue;
			}
			output.accept(id, supplier(converted.get(path).orElseThrow()));
		}
	}

	@Override
	public Set<String> getNamespaces(PackType type) {
		if (type != PackType.CLIENT_RESOURCES) {
			return Set.of();
		}
		PackArchive opened = archive();
		return opened == null ? Set.of() : PackConverter.namespaces(opened, this.descriptor);
	}

	@Override
	public void close() {
		synchronized (this.lock) {
			if (this.archive != null) {
				try {
					this.archive.close();
				} catch (IOException failure) {
					LOGGER.warn("failed to close legacy pack {}: {}", packId(), failure.getMessage());
				}
			}
			this.opened = false;
			this.archive = null;
			this.assets = null;
		}
	}

	private @Nullable PackArchive archive() {
		synchronized (this.lock) {
			if (!this.opened) {
				this.opened = true;
				try {
					this.archive = PackArchive.open(this.root.content());
				} catch (IOException failure) {
					LOGGER.error("failed to open legacy pack {}", this.root.content(), failure);
				}
			}
			return this.archive;
		}
	}

	private Assets assets() {
		synchronized (this.lock) {
			if (this.assets == null) {
				this.assets = convert();
			}
			return this.assets;
		}
	}

	private Assets convert() {
		PackArchive opened = archive();
		if (opened == null) {
			return new Assets(NativeImageDecoder.INSTANCE);
		}
		try {
			long start = System.nanoTime();
			Vanilla vanilla = new Vanilla(VanillaReader.client(), NativeImageDecoder.INSTANCE);
			Assets converted = PackConverter.convert(opened, this.descriptor, vanilla, NativeImageDecoder.INSTANCE);
			LOGGER.info("converted resource pack {} from format {} in {} ms", packId(), this.descriptor.format(), (System.nanoTime() - start) / 1_000_000);
			return converted;
		} catch (RuntimeException failure) {
			LOGGER.error("failed to convert legacy pack {}", packId(), failure);
			return new Assets(NativeImageDecoder.INSTANCE);
		}
	}

	private static IoSupplier<InputStream> supplier(Asset asset) {
		return () -> new ByteArrayInputStream(asset.bytes());
	}
}
