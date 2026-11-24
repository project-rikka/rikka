package sh.rikka.client.mixin;

import com.llamalad7.mixinextras.sugar.Local;
import java.nio.file.Path;
import net.minecraft.client.resources.server.DownloadedPackSource;
import net.minecraft.server.packs.repository.Pack;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyVariable;
import sh.rikka.client.resource.LegacyPacks;

@Mixin(DownloadedPackSource.class)
abstract class DownloadedPackSourceMixin {
	@ModifyVariable(method = "loadRequestedPacks", at = @At("STORE"))
	private Pack.ResourcesSupplier rikka$convertLegacyPack(Pack.ResourcesSupplier resources, @Local Path path) {
		return LegacyPacks.supplier(path, resources);
	}
}
