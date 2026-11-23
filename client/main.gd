extends Node

const ServerPacket = preload("res://server_packet.gd")
const World = preload("res://world.gd")
const INPUT_REPEAT_MS: int = 100
const HANDSHAKE_TIMEOUT_MS: int = 10_000

var _socket: WebSocketPeer
var _world: World
var _join_sent: bool = false
var _join_name: String
var _handshake_deadline_ms: int
var _server_error: String
var _seq: int
var _last_direction: Vector2i
var _next_repeat_ms: int
var _has_focus: bool = true

var _connection_screen: CenterContainer
var _ip_field: LineEdit
var _port_field: LineEdit
var _name_field: LineEdit
var _connect_button: Button
var _status: Label
var _notice: Label

# HUD Chiến đấu & Túi đồ (v0.2)
var _hud_layer: CanvasLayer
var _hud_container: VBoxContainer
var _player_info_label: Label
var _hp_bar: ProgressBar
var _exp_bar: ProgressBar
var _stats_label: Label
var _inventory_container: HBoxContainer
var _inventory_slots: Array[Button] = []
var _controls_hint: Label


func _ready() -> void:
	var overlay: CanvasLayer = CanvasLayer.new()
	add_child(overlay)

	_connection_screen = CenterContainer.new()
	_connection_screen.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	overlay.add_child(_connection_screen)
	var form: VBoxContainer = VBoxContainer.new()
	form.custom_minimum_size.x = 320
	form.add_theme_constant_override("separation", 8)
	_connection_screen.add_child(form)

	var title: Label = Label.new()
	title.text = "Deep Night"
	title.add_theme_font_size_override("font_size", 34)
	form.add_child(title)
	_ip_field = _add_field(form, "IP", "127.0.0.1")
	_port_field = _add_field(form, "Cổng", "7777")
	_name_field = _add_field(form, "Tên nhân vật", "")
	_name_field.text_submitted.connect(func(_value: String) -> void: _connect_to_server())

	_connect_button = Button.new()
	_connect_button.text = "Kết nối"
	_connect_button.pressed.connect(_connect_to_server)
	form.add_child(_connect_button)
	_status = Label.new()
	_status.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_status.custom_minimum_size.y = 52
	_status.text = "Nhập tên để tham gia."
	form.add_child(_status)
	_name_field.grab_focus()

	_notice = Label.new()
	_notice.position = Vector2(16, 12)
	_notice.add_theme_color_override("font_outline_color", Color("#16201b"))
	_notice.add_theme_constant_override("outline_size", 6)
	_notice.hide()
	overlay.add_child(_notice)

	_setup_hud()


func _setup_hud() -> void:
	_hud_layer = CanvasLayer.new()
	add_child(_hud_layer)

	# 1. Panel thông tin chỉ số người chơi (Góc trên bên trái)
	_hud_container = VBoxContainer.new()
	_hud_container.position = Vector2(16, 16)
	_hud_container.custom_minimum_size.x = 240
	_hud_container.add_theme_constant_override("separation", 4)
	_hud_layer.add_child(_hud_container)

	_player_info_label = Label.new()
	_player_info_label.text = "[Lv. 1] Hiệp sĩ"
	_player_info_label.add_theme_font_size_override("font_size", 16)
	_player_info_label.add_theme_color_override("font_outline_color", Color("#16201b"))
	_player_info_label.add_theme_constant_override("outline_size", 5)
	_hud_container.add_child(_player_info_label)

	_hp_bar = ProgressBar.new()
	_hp_bar.custom_minimum_size = Vector2(240, 18)
	_hp_bar.min_value = 0
	_hp_bar.max_value = 100
	_hp_bar.value = 100
	_hp_bar.show_percentage = false
	_hud_container.add_child(_hp_bar)

	_exp_bar = ProgressBar.new()
	_exp_bar.custom_minimum_size = Vector2(240, 12)
	_exp_bar.min_value = 0
	_exp_bar.max_value = 100
	_exp_bar.value = 0
	_exp_bar.show_percentage = false
	_hud_container.add_child(_exp_bar)

	_stats_label = Label.new()
	_stats_label.text = "HP: 100/100  |  EXP: 0/100  |  Công: 15  Thủ: 2"
	_stats_label.add_theme_font_size_override("font_size", 11)
	_stats_label.add_theme_color_override("font_outline_color", Color("#16201b"))
	_stats_label.add_theme_constant_override("outline_size", 4)
	_hud_container.add_child(_stats_label)

	# 2. Thanh Túi đồ Quickbar (Phía dưới màn hình)
	var bottom_container: VBoxContainer = VBoxContainer.new()
	bottom_container.set_anchors_and_offsets_preset(Control.PRESET_BOTTOM_WIDE)
	bottom_container.position.y = -70
	bottom_container.add_theme_constant_override("separation", 4)
	_hud_layer.add_child(bottom_container)

	_inventory_container = HBoxContainer.new()
	_inventory_container.alignment = BoxContainer.ALIGNMENT_CENTER
	_inventory_container.add_theme_constant_override("separation", 6)
	bottom_container.add_child(_inventory_container)

	for i: int in range(6):
		var btn: Button = Button.new()
		btn.custom_minimum_size = Vector2(100, 36)
		btn.text = "[%d] Trống" % (i + 1)
		var slot_idx: int = i
		btn.pressed.connect(func() -> void: _use_item(slot_idx))
		_inventory_container.add_child(btn)
		_inventory_slots.append(btn)

	_controls_hint = Label.new()
	_controls_hint.text = "[WASD]: Di chuyển  |  [Space]: Tấn công quái gần  |  [Phím 1-6]: Dùng đồ"
	_controls_hint.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	_controls_hint.add_theme_font_size_override("font_size", 12)
	_controls_hint.add_theme_color_override("font_outline_color", Color("#16201b"))
	_controls_hint.add_theme_constant_override("outline_size", 4)
	bottom_container.add_child(_controls_hint)

	_hud_layer.hide()


func _add_field(form: VBoxContainer, caption: String, value: String) -> LineEdit:
	var label: Label = Label.new()
	label.text = caption
	form.add_child(label)
	var field: LineEdit = LineEdit.new()
	field.text = value
	form.add_child(field)
	return field


func _connect_to_server() -> void:
	if _socket != null:
		return

	var ip: String = _ip_field.text.strip_edges()
	var port_text: String = _port_field.text.strip_edges()
	var port: int = port_text.to_int()
	var player_name: String = _name_field.text.strip_edges()
	if not ip.is_valid_ip_address():
		_status.text = "Địa chỉ IP không hợp lệ."
		return
	if not port_text.is_valid_int() or port < 1 or port > 65_535:
		_status.text = "Cổng phải là số nguyên từ 1 đến 65535."
		return
	if player_name.is_empty() or player_name.length() > 20:
		_status.text = "Tên phải dài từ 1 đến 20 ký tự."
		return
	for index: int in range(player_name.length()):
		var code: int = player_name.unicode_at(index)
		if code < 32 or (code >= 127 and code <= 159):
			_status.text = "Tên không được chứa ký tự điều khiển."
			return

	var host: String = "[%s]" % ip if ip.contains(":") else ip
	var url: String = "ws://%s:%d" % [host, port]
	_socket = WebSocketPeer.new()
	var result: Error = _socket.connect_to_url(url)
	if result != OK:
		_end_session("Không thể kết nối (%s)." % error_string(result))
		return

	_join_name = player_name
	_join_sent = false
	_server_error = ""
	_handshake_deadline_ms = Time.get_ticks_msec() + HANDSHAKE_TIMEOUT_MS
	_connect_button.disabled = true
	_status.text = "Đang kết nối tới %s…" % url


func _process(_delta: float) -> void:
	if _socket == null:
		return

	_socket.poll()
	var state: WebSocketPeer.State = _socket.get_ready_state()
	if state == WebSocketPeer.STATE_OPEN and not _join_sent:
		if not _send({"type": "Join", "name": _join_name}):
			return
		_join_sent = true
		_status.text = "Đã kết nối. Đang chờ vào bản đồ…"

	while _socket.get_available_packet_count() > 0:
		var bytes: PackedByteArray = _socket.get_packet()
		if _socket.get_packet_error() != OK or not _socket.was_string_packet():
			push_warning("Bỏ qua khung WebSocket không phải văn bản hợp lệ.")
			continue
		var packet: Dictionary = ServerPacket.decode(bytes.get_string_from_utf8())
		if not packet.is_empty():
			_handle_packet(packet)

	if state == WebSocketPeer.STATE_CLOSED:
		var code: int = _socket.get_close_code()
		var reason: String = _socket.get_close_reason()
		if code != -1:
			var close_message: String = "Máy chủ đã đóng kết nối."
			if _world == null and _join_sent:
				close_message = "Máy chủ đã đóng kết nối trước khi vào bản đồ."
			if code >= 1000 and code != 1005:
				close_message += " Mã đóng: %d." % code
			if not reason.is_empty():
				close_message += " " + reason
			_end_session(close_message)
		elif _world != null:
			_end_session("Đã mất kết nối với máy chủ.")
		elif _join_sent:
			_end_session("Đã mất kết nối trước khi vào bản đồ.")
		else:
			_end_session("Không thể mở kết nối tới máy chủ.")
	elif _world == null and Time.get_ticks_msec() >= _handshake_deadline_ms:
		if _join_sent:
			_end_session("Hết thời gian chờ vào bản đồ.")
		else:
			_end_session("Không thể mở kết nối trong thời gian chờ.")
	elif state == WebSocketPeer.STATE_OPEN and _world != null:
		_send_movement()


func _unhandled_input(event: InputEvent) -> void:
	if _world == null or not _has_focus:
		return

	if event is InputEventKey and event.pressed and not event.echo:
		match event.keycode:
			KEY_SPACE:
				_attack_nearest_monster()
			KEY_1:
				_use_item(0)
			KEY_2:
				_use_item(1)
			KEY_3:
				_use_item(2)
			KEY_4:
				_use_item(3)
			KEY_5:
				_use_item(4)
			KEY_6:
				_use_item(5)


func _attack_nearest_monster() -> void:
	if _world == null:
		return
	var target_id: int = _world.selected_target_id
	if target_id == 0:
		target_id = _world.get_nearest_monster(60.0)

	if target_id > 0:
		_seq += 1
		_send({
			"type": "Attack",
			"seq": _seq,
			"target_id": target_id,
		})


func _use_item(slot: int) -> void:
	_send({
		"type": "UseItem",
		"slot": slot,
	})


func _handle_packet(packet: Dictionary) -> void:
	var kind: String = packet["type"]
	if kind == "Error":
		_server_error = packet["message"]
		var message: String = "Lỗi từ máy chủ: %s" % _server_error
		if _world == null:
			_status.text = message
		else:
			_notice.text = message
			_notice.show()
		return

	if kind == "Welcome":
		if not _join_sent or _world != null:
			push_warning("Bỏ qua Welcome ngoài giai đoạn chờ vào bản đồ.")
			return
		_world = World.new()
		_world.local_id = int(packet["player_id"])
		_world.tick_rate = int(packet["tick_rate"])
		add_child(_world)
		_world.add_player(_world.local_id, _join_name)
		_seq = 0
		_last_direction = Vector2i.ZERO
		_next_repeat_ms = 0
		_server_error = ""
		_connection_screen.hide()
		_hud_layer.show()
		_player_info_label.text = "[Lv. 1] %s" % _join_name
		return

	if _world == null:
		push_warning("Bỏ qua %s trước Welcome." % kind)
		return

	match kind:
		"PlayerJoined":
			_world.add_player(int(packet["id"]), packet["name"])
		"PlayerLeft":
			_world.remove_player(int(packet["id"]))
		"PlayerStats":
			_update_hud_stats(packet)
		"Inventory":
			_update_hud_inventory(packet)
		"Snapshot":
			_world.apply_snapshot(
				packet["tick"],
				packet["positions"],
				packet.get("monsters", []),
				packet.get("npcs", []),
				packet.get("player_meta", {}),
				packet.get("combat_events", [])
			)


func _update_hud_stats(stats: Dictionary) -> void:
	var level: int = stats.get("level", 1)
	var exp: int = stats.get("exp", 0)
	var max_exp: int = stats.get("max_exp", 100)
	var hp: int = stats.get("hp", 100)
	var max_hp: int = stats.get("max_hp", 100)
	var atk: int = stats.get("attack_power", 15)
	var def: int = stats.get("defense", 2)

	_player_info_label.text = "[Lv. %d] %s" % [level, _join_name]

	_hp_bar.max_value = max_hp
	_hp_bar.value = hp

	_exp_bar.max_value = max_exp
	_exp_bar.value = exp

	_stats_label.text = "HP: %d/%d  |  EXP: %d/%d  |  Công: %d  Thủ: %d" % [
		hp, max_hp, exp, max_exp, atk, def
	]


func _update_hud_inventory(inv: Dictionary) -> void:
	var slots: Array = inv.get("slots", [])
	var slot_map: Dictionary = {}
	for item: Variant in slots:
		if item is Dictionary:
			slot_map[int(item["slot"])] = item

	for i: int in range(_inventory_slots.size()):
		var btn: Button = _inventory_slots[i]
		if slot_map.has(i):
			var item: Dictionary = slot_map[i]
			var item_id: String = item["item_id"]
			var display_name: String = "Bình máu" if item_id == "potion_hp" else (
				"Tai yêu tinh" if item_id == "goblin_ear" else (
					"Da sói" if item_id == "wolf_pelt" else item_id
				)
			)
			btn.text = "[%d] %s (%d)" % [i + 1, display_name, item["count"]]
		else:
			btn.text = "[%d] Trống" % (i + 1)


func _notification(what: int) -> void:
	if what == NOTIFICATION_APPLICATION_FOCUS_OUT:
		_has_focus = false
	elif what == NOTIFICATION_APPLICATION_FOCUS_IN:
		_has_focus = true


func _send_movement() -> void:
	var direction: Vector2i = Vector2i.ZERO
	if _has_focus:
		direction.x = (
			int(Input.is_action_pressed("move_right")) - int(Input.is_action_pressed("move_left"))
		)
		direction.y = (
			int(Input.is_action_pressed("move_down")) - int(Input.is_action_pressed("move_up"))
		)

	var now_ms: int = Time.get_ticks_msec()
	var direction_changed: bool = direction != _last_direction
	var repeat_due: bool = direction != Vector2i.ZERO and now_ms >= _next_repeat_ms
	if not direction_changed and not repeat_due:
		return

	_seq += 1
	if _send({"type": "Input", "seq": _seq, "dx": direction.x, "dy": direction.y}):
		_last_direction = direction
		_next_repeat_ms = now_ms + INPUT_REPEAT_MS


func _send(packet: Dictionary) -> bool:
	var result: Error = _socket.send_text(JSON.stringify(packet))
	if result != OK:
		_end_session("Không thể gửi gói %s (%s)." % [packet["type"], error_string(result)])
		return false
	return true


func _end_session(reason: String) -> void:
	if _socket != null:
		_socket.close(-1)
		_socket = null
	if _world != null:
		remove_child(_world)
		_world.queue_free()
		_world = null
	_hud_layer.hide()
	_notice.hide()
	_connection_screen.show()
	_connect_button.disabled = false
	_status.text = reason.strip_edges()
	if not _server_error.is_empty():
		_status.text += "\nLỗi từ máy chủ: %s" % _server_error
	_server_error = ""
	_name_field.grab_focus()
