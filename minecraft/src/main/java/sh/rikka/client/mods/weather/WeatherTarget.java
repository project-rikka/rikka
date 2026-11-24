package sh.rikka.client.mods.weather;

import net.minecraft.world.level.biome.Biome;
import org.jspecify.annotations.Nullable;

record WeatherTarget(float rain, float thunder, Biome.@Nullable Precipitation precipitation, int lightningPerMinute) {
}
