package sh.rikka.client.resource;

import java.io.IOException;
import java.io.InputStream;
import java.util.Optional;
import net.minecraft.client.Minecraft;
import net.minecraft.resources.Identifier;
import net.minecraft.server.packs.PackResources;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.resources.IoSupplier;
import sh.rikka.client.resource.legacy.Assets;
import sh.rikka.client.resource.legacy.Vanilla;

final class VanillaReader implements Vanilla.Reader {
	private final PackResources resources;

	private VanillaReader(PackResources resources) {
		this.resources = resources;
	}

	static VanillaReader client() {
		return new VanillaReader(Minecraft.getInstance().getVanillaPackResources().fullResources());
	}

	@Override
	public Optional<byte[]> read(String path) throws IOException {
		if (!path.startsWith(Assets.ROOT)) {
			return Optional.empty();
		}
		int separator = path.indexOf('/', Assets.ROOT.length());
		if (separator < 0) {
			return Optional.empty();
		}
		Identifier id = Identifier.tryBuild(path.substring(Assets.ROOT.length(), separator), path.substring(separator + 1));
		if (id == null) {
			return Optional.empty();
		}
		IoSupplier<InputStream> resource = this.resources.getResource(PackType.CLIENT_RESOURCES, id);
		if (resource == null) {
			return Optional.empty();
		}
		try (InputStream in = resource.get()) {
			return Optional.of(in.readAllBytes());
		}
	}
}
