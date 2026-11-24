package sh.rikka.client.config;

import com.google.common.net.HostAndPort;
import com.google.common.net.InetAddresses;
import com.google.common.net.InternetDomainName;
import com.mojang.serialization.Codec;
import com.mojang.serialization.DataResult;
import java.net.IDN;
import java.util.Comparator;
import java.util.Locale;
import java.util.Objects;
import java.util.Optional;
import java.util.OptionalInt;

public final class ServerPattern {
	public static final Codec<ServerPattern> CODEC = Codec.STRING.comapFlatMap(
		text -> parse(text).map(DataResult::success).orElseGet(() -> DataResult.error(() -> "invalid server address " + text)),
		ServerPattern::toString
	);
	static final Comparator<ServerPattern> SPECIFICITY = Comparator.comparingInt(ServerPattern::labels)
		.thenComparing(pattern -> pattern.port.isPresent());

	private static final int MIN_PORT = 1;
	private static final char LABEL_SEPARATOR = '.';

	private final Host host;
	private final OptionalInt port;

	private ServerPattern(Host host, OptionalInt port) {
		this.host = host;
		this.port = port;
	}

	public static Optional<ServerPattern> parse(String text) {
		HostAndPort parsed;
		try {
			parsed = HostAndPort.fromString(text.strip());
		} catch (IllegalArgumentException invalid) {
			return Optional.empty();
		}
		OptionalInt port = parsed.hasPort() ? OptionalInt.of(parsed.getPort()) : OptionalInt.empty();
		if (port.isPresent() && port.getAsInt() < MIN_PORT) {
			return Optional.empty();
		}
		return Host.of(parsed.getHost())
			.filter(host -> host.kind() == Kind.IP || InternetDomainName.isValid(host.name()))
			.map(host -> new ServerPattern(host, port));
	}

	boolean matches(Host candidate, int candidatePort) {
		if (this.port.isPresent() && this.port.getAsInt() != candidatePort) {
			return false;
		}
		if (candidate.kind() != this.host.kind()) {
			return false;
		}
		String name = this.host.name();
		return switch (this.host.kind()) {
			case IP -> candidate.name().equals(name);
			case DOMAIN -> candidate.name().equals(name) || candidate.name().endsWith(LABEL_SEPARATOR + name);
		};
	}

	@Override
	public boolean equals(Object other) {
		return other instanceof ServerPattern pattern && pattern.host.equals(this.host) && pattern.port.equals(this.port);
	}

	@Override
	public int hashCode() {
		return Objects.hash(this.host, this.port);
	}

	@Override
	public String toString() {
		String name = this.host.name();
		HostAndPort address = this.port.isPresent() ? HostAndPort.fromParts(name, this.port.getAsInt()) : HostAndPort.fromHost(name);
		return address.toString();
	}

	private int labels() {
		return switch (this.host.kind()) {
			case IP -> 1;
			case DOMAIN -> (int) this.host.name().chars().filter(character -> character == LABEL_SEPARATOR).count() + 1;
		};
	}

	enum Kind {
		DOMAIN,
		IP
	}

	record Host(String name, Kind kind) {
		static Optional<Host> of(String host) {
			if (InetAddresses.isInetAddress(host)) {
				return Optional.of(new Host(InetAddresses.toAddrString(InetAddresses.forString(host)), Kind.IP));
			}
			String rooted = host.endsWith(String.valueOf(LABEL_SEPARATOR)) ? host.substring(0, host.length() - 1) : host;
			if (rooted.isEmpty()) {
				return Optional.empty();
			}
			try {
				return Optional.of(new Host(IDN.toASCII(rooted).toLowerCase(Locale.ROOT), Kind.DOMAIN));
			} catch (IllegalArgumentException invalid) {
				return Optional.empty();
			}
		}
	}
}
