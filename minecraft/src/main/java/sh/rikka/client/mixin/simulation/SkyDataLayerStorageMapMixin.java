package sh.rikka.client.mixin.simulation;

import it.unimi.dsi.fastutil.longs.Long2ObjectOpenHashMap;
import net.minecraft.world.level.chunk.DataLayer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Redirect;

@Mixin(targets = "net.minecraft.world.level.lighting.SkyLightSectionStorage$SkyDataLayerStorageMap")
abstract class SkyDataLayerStorageMapMixin {
	@Redirect(
		method = "copy",
		at = @At(
			value = "INVOKE",
			target = "Lit/unimi/dsi/fastutil/longs/Long2ObjectOpenHashMap;clone()Lit/unimi/dsi/fastutil/longs/Long2ObjectOpenHashMap;"
		)
	)
	private Long2ObjectOpenHashMap<DataLayer> rikka$snapshotLayers(Long2ObjectOpenHashMap<DataLayer> layers) {
		return new Long2ObjectOpenHashMap<>(0);
	}
}
