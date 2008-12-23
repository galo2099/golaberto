module UserHelper
  def precise_distance_of_time(from_date, to_date)
    from_date = from_date.to_time
    to_date = to_date.to_time
    distance = { :years => 0, :months => 0, :days => 0, :hours => 0, :minutes => 0, :seconds => 0 }
    [:years, :months, :days, :hours, :minutes, :seconds].each do |period|
      while to_date - 1.send(period) >= from_date do
        distance[period] += 1
        to_date -= 1.send(period)
      end
    end
    distance
  end

  def precise_time_ago(from_date)
    precise_distance_of_time(from_date, Time.now)
  end

  def precise_distance_of_time_in_words(from_date, to_date)
    distance = precise_distance_of_time(from_date, to_date)
    str = ""
    if distance[:years] > 0
      str << n_("%{num} year", "%{num} years", distance[:years]) % { :num => distance[:years] }
    end
    if distance[:months] > 0
      str << ", " unless str.empty?
      str << n_("%{num} month", "%{num} months", distance[:months]) % { :num => distance[:months] }
    end
    if distance[:days] > 0
      str << ", " unless str.empty?
      str << n_("%{num} day", "%{num} days", distance[:days]) % { :num => distance[:days] }
    end
    str
  end

  def precise_time_ago_in_words(from_date)
    precise_distance_of_time_in_words(from_date, Time.now)
  end
end
