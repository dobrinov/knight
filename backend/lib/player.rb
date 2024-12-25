class Player
  attr_reader :name, :wood, :stone, :iron, :gold

  def initialize(id:, name:, wood: 0, stone: 0, iron: 0, gold: 0)
    @name = name
    @wood = wood
    @stone = stone
    @iron = iron
    @gold = gold
  end

  def give_resource(resource:, amount:)
    case resource
    when Resource::WOOD
      @wood += amount
    when Resource::STONE
      @stone += amount
    when Resource::IRON
      @iron += amount
    when Resource::GOLD
      @gold += amount
    end
  end

  def to_s
    @id.to_s
  end
end
