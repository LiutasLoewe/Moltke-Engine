extends Node
class_name SimulationClock

## Fixed-step simulation clock. Frame time controls scheduling only; simulation
## state advances exclusively through integer ticks.
signal tick_advanced(tick_index: int, year: int, month: int, day: int)
signal month_advanced(year: int, month: int)
signal year_advanced(year: int)
signal pause_changed(is_paused: bool)
signal speed_changed(multiplier: float)

const SPEED_STEPS: Array[float] = [1.0, 2.0, 4.0, 8.0]
const MAX_CATCH_UP_STEPS := 30

@export_range(0.1, 120.0, 0.1) var ticks_per_second: float = 4.0

var simulation_tick: int = 0
var year: int = 1
var month: int = 1
var day: int = 1
var speed_multiplier: float = 1.0
var is_paused: bool = false

var _accumulator: float = 0.0


func _process(delta: float) -> void:
	if is_paused or ticks_per_second <= 0.0:
		return

	_accumulator += maxf(delta, 0.0) * speed_multiplier
	var tick_duration := 1.0 / ticks_per_second
	var catch_up_steps := 0
	while _accumulator >= tick_duration and catch_up_steps < MAX_CATCH_UP_STEPS:
		_accumulator -= tick_duration
		_advance_one_tick()
		catch_up_steps += 1


func start_new_game(start_year: int = 1, start_month: int = 1, start_day: int = 1) -> bool:
	if start_year < 1 or start_month < 1 or start_month > 12:
		return false
	if start_day < 1 or start_day > _days_in_month(start_year, start_month):
		return false

	simulation_tick = 0
	year = start_year
	month = start_month
	day = start_day
	_accumulator = 0.0
	return true


func set_paused(value: bool) -> void:
	if is_paused == value:
		return
	is_paused = value
	pause_changed.emit(is_paused)


func set_speed(multiplier: float) -> bool:
	if not SPEED_STEPS.has(multiplier):
		return false
	if is_equal_approx(speed_multiplier, multiplier):
		return true
	speed_multiplier = multiplier
	speed_changed.emit(speed_multiplier)
	return true


## Advances whole simulation ticks directly, independent of rendered frames.
func advance_ticks(count: int = 1) -> bool:
	if count < 0:
		return false
	for _index in range(count):
		_advance_one_tick()
	return true


func to_state() -> Dictionary:
	return {
		"simulation_tick": simulation_tick,
		"year": year,
		"month": month,
		"day": day,
		"ticks_per_second": ticks_per_second,
		"speed_multiplier": speed_multiplier,
		"is_paused": is_paused
	}


func load_state(state: Dictionary) -> bool:
	var raw_tick: Variant = state.get("simulation_tick", null)
	var raw_year: Variant = state.get("year", null)
	var raw_month: Variant = state.get("month", null)
	var raw_day: Variant = state.get("day", null)
	var raw_rate: Variant = state.get("ticks_per_second", null)
	var raw_speed: Variant = state.get("speed_multiplier", null)
	var raw_paused: Variant = state.get("is_paused", null)

	if typeof(raw_tick) != TYPE_INT or int(raw_tick) < 0:
		return false
	if typeof(raw_year) != TYPE_INT or int(raw_year) < 1:
		return false
	if typeof(raw_month) != TYPE_INT or int(raw_month) < 1 or int(raw_month) > 12:
		return false
	if typeof(raw_day) != TYPE_INT or int(raw_day) < 1:
		return false
	if int(raw_day) > _days_in_month(int(raw_year), int(raw_month)):
		return false
	if typeof(raw_rate) != TYPE_FLOAT and typeof(raw_rate) != TYPE_INT:
		return false
	if float(raw_rate) <= 0.0:
		return false
	if typeof(raw_speed) != TYPE_FLOAT and typeof(raw_speed) != TYPE_INT:
		return false
	if not SPEED_STEPS.has(float(raw_speed)):
		return false
	if typeof(raw_paused) != TYPE_BOOL:
		return false

	var previous_speed := speed_multiplier
	var previous_pause_state := is_paused
	simulation_tick = int(raw_tick)
	year = int(raw_year)
	month = int(raw_month)
	day = int(raw_day)
	ticks_per_second = float(raw_rate)
	speed_multiplier = float(raw_speed)
	is_paused = bool(raw_paused)
	_accumulator = 0.0
	if not is_equal_approx(previous_speed, speed_multiplier):
		speed_changed.emit(speed_multiplier)
	if previous_pause_state != is_paused:
		pause_changed.emit(is_paused)
	return true


func _advance_one_tick() -> void:
	simulation_tick += 1
	var previous_year := year
	var previous_month := month

	day += 1
	if day > _days_in_month(year, month):
		day = 1
		month += 1
		if month > 12:
			month = 1
			year += 1

	if year != previous_year:
		year_advanced.emit(year)
	if month != previous_month:
		month_advanced.emit(year, month)
	tick_advanced.emit(simulation_tick, year, month, day)


func _days_in_month(target_year: int, target_month: int) -> int:
	match target_month:
		2:
			return 29 if _is_leap_year(target_year) else 28
		4, 6, 9, 11:
			return 30
		_:
			return 31


func _is_leap_year(target_year: int) -> bool:
	return target_year % 400 == 0 or (target_year % 4 == 0 and target_year % 100 != 0)
