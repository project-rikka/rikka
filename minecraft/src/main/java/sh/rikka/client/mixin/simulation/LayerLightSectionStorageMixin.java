package sh.rikka.client.mixin.simulation;

import net.minecraft.world.level.lighting.DataLayerStorageMap;
import net.minecraft.world.level.lighting.LayerLightSectionStorage;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Redirect;
import sh.rikka.client.simulation.light.SnapshotLayerStorage;

@Mixin(LayerLightSectionStorage.class)
abstract class LayerLightSectionStorageMixin {
	@Redirect(
		method = {"<init>", "swapSectionMap"},
		at = @At(
			value = "INVOKE",
			target = "Lnet/minecraft/world/level/lighting/DataLayerStorageMap;copy()Lnet/minecraft/world/level/lighting/DataLayerStorageMap;"
		)
	)
	private DataLayerStorageMap<?> rikka$snapshot(DataLayerStorageMap<?> updating) {
		DataLayerStorageMap<?> snapshot = updating.copy();
		((SnapshotLayerStorage) snapshot).rikka$view(((SnapshotLayerStorage) updating).rikka$publish());
		return snapshot;
	}
}
