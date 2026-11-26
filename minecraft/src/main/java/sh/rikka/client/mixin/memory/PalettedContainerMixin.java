package sh.rikka.client.mixin.memory;

import net.minecraft.util.ThreadingDetector;
import net.minecraft.util.ZeroBitStorage;
import net.minecraft.world.level.chunk.PalettedContainer;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Mutable;
import org.spongepowered.asm.mixin.Overwrite;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Redirect;
import sh.rikka.client.memory.PalettedContainerAccess;
import sh.rikka.client.memory.ZeroBitStorages;

@Mixin(PalettedContainer.class)
abstract class PalettedContainerMixin {
	@Shadow
	@Final
	@Mutable
	private ThreadingDetector threadingDetector;

	@Redirect(method = "<init>*", at = @At(value = "NEW", target = "net/minecraft/util/ThreadingDetector"), require = 3)
	private ThreadingDetector rikka$detectOnContention(String name) {
		return PalettedContainerAccess.FREE;
	}

	@Redirect(method = "createOrReuseData", at = @At(value = "NEW", target = "net/minecraft/util/ZeroBitStorage"))
	private ZeroBitStorage rikka$sharedZeroBitStorage(int size) {
		return ZeroBitStorages.ofSize(size);
	}

	@Redirect(method = "unpack", at = @At(value = "NEW", target = "net/minecraft/util/ZeroBitStorage"))
	private static ZeroBitStorage rikka$sharedUnpackedZeroBitStorage(int size) {
		return ZeroBitStorages.ofSize(size);
	}

	@Overwrite
	public void acquire() {
		PalettedContainerAccess.acquire((PalettedContainer<?>) (Object) this);
	}

	@Overwrite
	public void release() {
		PalettedContainerAccess.release((PalettedContainer<?>) (Object) this);
	}
}
