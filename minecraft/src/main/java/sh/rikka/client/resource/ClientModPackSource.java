package sh.rikka.client.resource;

import java.nio.file.Path;
import java.util.List;
import java.util.Optional;
import java.util.function.Consumer;
import net.fabricmc.loader.api.FabricLoader;
import net.fabricmc.loader.api.ModContainer;
import net.minecraft.network.chat.Component;
import net.minecraft.server.packs.PackLocationInfo;
import net.minecraft.server.packs.PackSelectionConfig;
import net.minecraft.server.packs.PathPackResources;
import net.minecraft.server.packs.repository.Pack;
import net.minecraft.server.packs.repository.PackCompatibility;
import net.minecraft.server.packs.repository.PackSource;
import net.minecraft.server.packs.repository.RepositorySource;
import net.minecraft.world.flag.FeatureFlagSet;
import sh.rikka.client.RikkaClient;

public final class ClientModPackSource implements RepositorySource {
	private static final PackSelectionConfig SELECTION = new PackSelectionConfig(true, Pack.Position.BOTTOM, true);

	@Override
	public void loadPacks(Consumer<Pack> result) {
		Component title = Component.literal(RikkaClient.NAME);
		PackLocationInfo location = new PackLocationInfo(RikkaClient.MOD_ID, title, PackSource.BUILT_IN, Optional.empty());
		Pack.Metadata metadata = new Pack.Metadata(title, PackCompatibility.COMPATIBLE, FeatureFlagSet.of(), List.of());
		result.accept(new Pack(location, new PathPackResources.PathResourcesSupplier(root()), metadata, SELECTION));
	}

	private static Path root() {
		ModContainer mod = FabricLoader.getInstance()
			.getModContainer(RikkaClient.MOD_ID)
			.orElseThrow(() -> new IllegalStateException(RikkaClient.MOD_ID + " is not loaded"));
		List<Path> roots = mod.getRootPaths();
		if (roots.size() != 1) {
			throw new IllegalStateException(RikkaClient.MOD_ID + " has " + roots.size() + " root paths, expected one");
		}
		return roots.getFirst();
	}
}
