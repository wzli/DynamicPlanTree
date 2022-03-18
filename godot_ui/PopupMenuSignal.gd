extends MenuButton

signal id_pressed(id)

func _ready():
	get_popup().connect("id_pressed",self,"_on_id_pressed")

func _on_id_pressed(id : int):
	emit_signal("id_pressed", id)
