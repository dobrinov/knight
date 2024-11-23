class Piece
  attr_reader :player, :attack, :defense, :health

  class << self
    def parse(value)
      raise 'Invalid value size' unless value.size == 4

      type = value[0]
      attack, defense, health = value[1, 3].split('').map(&:to_i)

      klass =
        case type
        when Pawn::TYPE
          Pawn
        when Rook::TYPE
          Rook
        when Knight::TYPE
          Knight
        when Bishop::TYPE
          Bishop
        when Queen::TYPE
          Queen
        when King::TYPE
          King
        else
          raise "Unknown piece type: #{type}"
        end

      klass.new attack:, defense:, health:
    end
  end

  def initialize(attack: nil, defense: nil, health: nil)
    @attack = attack || self.class::ATTACK
    @defense = defense || self.class::DEFENSE
    @health = health || self.class::HEALTH
  end

  def receive_attack(attack)
    damage_taken = [attack - self.defense, 1].max
    @health = [@health - damage_taken, 0].max
  end

  def alive?
    @health > 0
  end

  def move_pattern
    unless self.class.const_defined?(:MOVE_PATTERN)
      raise NotImplementedError, "#{self.class} must define a MOVE_PATTERN constant"
    end

    self.class::MOVE_PATTERN
  end

  def move_pattern_repetition_limit
    unless self.class.const_defined?(:MOVE_PATTERN_REPETITION_LIMIT)
      raise NotImplementedError, "#{self.class} must define a MOVE_PATTERN_REPETITION_LIMIT constant"
    end

    self.class::MOVE_PATTERN_REPETITION_LIMIT
  end

  def to_s
    "#{self.class::TYPE}#{@attack}#{@defense}#{@health}"
  end
end
