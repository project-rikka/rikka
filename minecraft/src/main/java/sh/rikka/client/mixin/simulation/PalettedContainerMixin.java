package sh.rikka.client.mixin.simulation;

import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import com.llamalad7.mixinextras.sugar.Local;
import it.unimi.dsi.fastutil.ints.Int2IntOpenHashMap;
import java.util.function.IntConsumer;
import net.minecraft.util.BitStorage;
import net.minecraft.world.level.chunk.Palette;
import net.minecraft.world.level.chunk.PalettedContainer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Redirect;
import sh.rikka.client.simulation.chunk.PaletteHistogram;
import sh.rikka.client.simulation.chunk.PaletteReencoder;

@Mixin(PalettedContainer.class)
abstract class PalettedContainerMixin {
	@Redirect(
		method = "pack",
		at = @At(
			value = "INVOKE",
			target = "Lnet/minecraft/world/level/chunk/PalettedContainer;reencodeContents(Lnet/minecraft/util/BitStorage;Lnet/minecraft/world/level/chunk/Palette;Lnet/minecraft/world/level/chunk/Palette;)[I"
		)
	)
	private <T> int[] rikka$packContents(BitStorage storage, Palette<T> oldPalette, Palette<T> newPalette) {
		return PaletteReencoder.reencode(storage, oldPalette, newPalette);
	}

	@Redirect(
		method = "unpack",
		at = @At(
			value = "INVOKE",
			target = "Lnet/minecraft/world/level/chunk/PalettedContainer;reencodeContents(Lnet/minecraft/util/BitStorage;Lnet/minecraft/world/level/chunk/Palette;Lnet/minecraft/world/level/chunk/Palette;)[I"
		)
	)
	private static <T> int[] rikka$unpackContents(BitStorage storage, Palette<T> oldPalette, Palette<T> newPalette) {
		return PaletteReencoder.reencode(storage, oldPalette, newPalette);
	}

	@WrapOperation(
		method = "count",
		at = @At(value = "INVOKE", target = "Lnet/minecraft/util/BitStorage;getAll(Ljava/util/function/IntConsumer;)V")
	)
	private void rikka$countIds(BitStorage storage, IntConsumer output, Operation<Void> original, @Local Int2IntOpenHashMap counts) {
		if (PaletteHistogram.supports(storage)) {
			PaletteHistogram.countInto(storage, counts);
		} else {
			original.call(storage, output);
		}
	}
}
