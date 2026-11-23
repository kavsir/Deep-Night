extends RefCounted

# JSON numbers parsed by Godot are floats. Reject IDs and ticks that might alias.
const MAX_EXACT_JSON_INT: int = 9_007_199_254_740_991
const MAX_U32: int = 4_294_967_295
const MAX_F32: float = 3.4028234663852886e38


static func decode(text: String) -> Dictionary:
	var json: JSON = JSON.new()
	if json.parse(text) != OK or not json.data is Dictionary:
		push_warning("Bỏ qua gói tin có JSON không hợp lệ.")
		return {}

	var packet: Dictionary = json.data
	match packet.get("type"):
		"Welcome":
			if (
				_is_uint(packet.get("player_id"))
				and _is_uint(packet.get("tick_rate"), MAX_U32)
				and packet["tick_rate"] > 0
				and packet.get("map") is String
			):
				return packet
		"PlayerJoined":
			if _is_uint(packet.get("id")) and packet.get("name") is String:
				return packet
		"PlayerLeft":
			if _is_uint(packet.get("id")):
				return packet
		"Snapshot":
			if _is_uint(packet.get("tick")) and packet.get("players") is Array:
				var positions: Dictionary = {}
				for player: Variant in packet["players"]:
					if (
						not player is Dictionary
						or not _is_uint(player.get("id"))
						or not _is_coordinate(player.get("x"))
						or not _is_coordinate(player.get("y"))
						or positions.has(int(player["id"]))
					):
						push_warning("Bỏ qua Snapshot có dữ liệu người chơi không hợp lệ.")
						return {}
					positions[int(player["id"])] = Vector2(player["x"], player["y"])
				return {"type": "Snapshot", "tick": int(packet["tick"]), "positions": positions}
		"Error":
			if packet.get("message") is String:
				return packet

	push_warning("Bỏ qua gói tin có loại hoặc trường không hợp lệ.")
	return {}


static func _is_uint(value: Variant, maximum: int = MAX_EXACT_JSON_INT) -> bool:
	if not (value is float or value is int):
		return false
	return is_finite(value) and value >= 0 and value <= maximum and floor(value) == value


static func _is_coordinate(value: Variant) -> bool:
	if not (value is float or value is int):
		return false
	return is_finite(value) and absf(value) <= MAX_F32
