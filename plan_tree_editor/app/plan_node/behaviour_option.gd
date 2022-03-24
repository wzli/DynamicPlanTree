extends OptionButton


func _ready():
	update_schema()


func set_behaviour(name):
	var behaviours = Global.schema["BehaviourEnum"]["ENUM"]
	for idx in behaviours:
		for found_name in behaviours[idx]:
			if name == found_name:
				select(int(idx))


func update_schema():
	var prev_selected = get_item_text(selected)
	var behaviours = Global.schema["BehaviourEnum"]["ENUM"]
	clear()
	for idx in behaviours:
		for name in behaviours[idx]:
			add_item(name)
			if name == prev_selected:
				select(get_item_count() - 1)
	if get_selected_id() == -1:
		select(0)
