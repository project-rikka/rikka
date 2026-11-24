package sh.rikka.client.mixin;

import java.nio.file.Path;
import java.util.function.BiConsumer;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.repository.FolderRepositorySource;
import net.minecraft.server.packs.repository.Pack;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyArg;
import sh.rikka.client.resource.LegacyPacks;

@Mixin(FolderRepositorySource.class)
abstract class FolderRepositorySourceMixin {
	@Shadow
	@Final
	private PackType packType;

	@ModifyArg(
		method = "loadPacks",
		at = @At(
			value = "INVOKE",
			target = "Lnet/minecraft/server/packs/repository/FolderRepositorySource;discoverPacks(Ljava/nio/file/Path;Lnet/minecraft/world/level/validation/DirectoryValidator;Ljava/util/function/BiConsumer;)V"
		),
		index = 2
	)
	private BiConsumer<Path, Pack.ResourcesSupplier> rikka$convertLegacyPacks(BiConsumer<Path, Pack.ResourcesSupplier> result) {
		if (this.packType != PackType.CLIENT_RESOURCES) {
			return result;
		}
		return (content, resources) -> result.accept(content, LegacyPacks.supplier(content, resources));
	}
}
