extends OptionButton

var schema_version;

func _ready():
	reload_schema()

func _pressed():
	reload_schema()

func set_behaviour(name):
	if not Global.schema:
		return
	var behaviours = Global.schema["BehaviourEnum"]["ENUM"]
	for idx in behaviours:
		for found_name in behaviours[idx]:
			if name == found_name:
				select(int(idx))

func reload_schema():
	if schema_version != Global.schema_version:
		schema_version = Global.schema_version
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
