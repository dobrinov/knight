module Resource
  module_function

  ALL = [
    WOOD = 'W',
    STONE = 'S',
    IRON = 'I',
    GOLD = 'G'
  ]

  def valid?(value)
    return true if value.nil?

    ALL.include? value
  end

  def parse(value)
    case value
    when '-'
      nil
    when *ALL
      value
    else
      raise ArgumentError, "Invalid resource: #{value}"
    end
  end
end
