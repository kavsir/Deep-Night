extends Node2D

const MAP_SIZE: Vector2i = Vector2i(1024, 1024)
const TILE_SIZE: int = 32
const TILE_COLORS: Array[Color] = [Color("#3a5144"), Color("#425b4b")]
const LOCAL_COLOR: Color = Color("#f3c979")
const REMOTE_COLOR: Color = Color("#9eddf3")

var local_id: int
var tick_rate: int

var _players: Dictionary = {}
var _camera: Camera2D
var _previous_positions: Dictionary = {}
var _latest_positions: Dictionary = {}
var _latest_tick: int = -1
var _snapshot_time_us: int
var _snapshot_interval: float = 0.0


func add_player(id: int, player_name: String) -> void:
	if _players.has(id):
		return

	var player: Node2D = Node2D.new()
	player.hide()
	add_child(player)
	_players[id] = player

	var body: Polygon2D = Polygon2D.new()
	body.polygon = PackedVector2Array([
		Vector2(-11, -11), Vector2(11, -11), Vector2(11, 11), Vector2(-11, 11)
	])
	body.color = LOCAL_COLOR if id == local_id else REMOTE_COLOR
	player.add_child(body)

	var name_label: Label = Label.new()
	name_label.text = player_name
	name_label.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	name_label.add_theme_color_override("font_outline_color", Color("#16201b"))
	name_label.add_theme_constant_override("outline_size", 5)
	player.add_child(name_label)
	var label_size: Vector2 = name_label.get_minimum_size()
	name_label.position = Vector2(-label_size.x / 2.0, -16.0 - label_size.y)

	if id == local_id:
		_camera = Camera2D.new()
		_camera.enabled = false
		player.add_child(_camera)

	_update_player(id, player, _interpolation_weight())


func remove_player(id: int) -> void:
	var player: Node2D = _players.get(id)
	if player == null:
		return
	player.hide()
	player.queue_free()
	_players.erase(id)
	_previous_positions.erase(id)
	_latest_positions.erase(id)


func apply_snapshot(tick: int, positions: Dictionary) -> void:
	if tick <= _latest_tick:
		return

	_snapshot_interval = float(tick - _latest_tick) / tick_rate if _latest_tick >= 0 else 0.0
	_previous_positions = _latest_positions
	_latest_positions = positions
	_latest_tick = tick
	_snapshot_time_us = Time.get_ticks_usec()


func _process(_delta: float) -> void:
	var weight: float = _interpolation_weight()
	for id: int in _players:
		_update_player(id, _players[id], weight)


func _interpolation_weight() -> float:
	if _snapshot_interval <= 0.0:
		return 1.0
	var elapsed: float = (Time.get_ticks_usec() - _snapshot_time_us) / 1_000_000.0
	return clampf(elapsed / _snapshot_interval, 0.0, 1.0)


func _update_player(id: int, player: Node2D, weight: float) -> void:
	if not _latest_positions.has(id):
		return
	var latest: Vector2 = _latest_positions[id]
	var previous: Vector2 = _previous_positions.get(id, latest)
	player.position = previous.lerp(latest, weight)
	if not player.visible:
		player.show()
		if id == local_id:
			_camera.enabled = true


func _draw() -> void:
	for row: int in range(ceili(float(MAP_SIZE.y) / TILE_SIZE)):
		for column: int in range(ceili(float(MAP_SIZE.x) / TILE_SIZE)):
			var tile: Rect2 = Rect2(column * TILE_SIZE, row * TILE_SIZE, TILE_SIZE, TILE_SIZE)
			draw_rect(tile, TILE_COLORS[(row + column) % 2])
