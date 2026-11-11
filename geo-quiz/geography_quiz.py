QUESTIONS = [
    {
        "question": "What is the largest continent by area?",
        "options": ["Africa", "Asia", "Europe", "North America"],
        "answer": 2,
    },
    {
        "question": "What is the capital city of Australia?",
        "options": ["Sydney", "Melbourne", "Canberra", "Perth"],
        "answer": 3,
    },
    {
        "question": "Which river is generally considered the longest in the world?",
        "options": ["Amazon River", "Nile River", "Yangtze River", "Mississippi River"],
        "answer": 2,
    },
    {
        "question": "Mount Everest is located in which mountain range?",
        "options": ["Andes", "Alps", "Himalayas", "Rockies"],
        "answer": 3,
    },
    {
        "question": "Which country has the largest land area?",
        "options": ["Canada", "China", "United States", "Russia"],
        "answer": 4,
    },
    {
        "question": "Which ocean lies between Africa, Asia, and Australia?",
        "options": ["Atlantic Ocean", "Pacific Ocean", "Indian Ocean", "Arctic Ocean"],
        "answer": 3,
    },
    {
        "question": "What is the smallest independent country in the world?",
        "options": ["Monaco", "Vatican City", "San Marino", "Liechtenstein"],
        "answer": 2,
    },
    {
        "question": "The Sahara Desert is located primarily on which continent?",
        "options": ["Asia", "South America", "Africa", "Australia"],
        "answer": 3,
    },
    {
        "question": 'Which country is known as the "Land of the Rising Sun"?',
        "options": ["China", "South Korea", "Thailand", "Japan"],
        "answer": 4,
    },
    {
        "question": "What imaginary line divides Earth into the Northern and Southern Hemispheres?",
        "options": ["Prime Meridian", "Tropic of Cancer", "Equator", "International Date Line"],
        "answer": 3,
    },
]


def ask_question(number, question_data):
    print(f"\nQuestion {number}: {question_data['question']}")
    for option_number, option in enumerate(question_data["options"], start=1):
        print(f"  {option_number}. {option}")

    while True:
        choice = input("Your answer (1-4): ").strip()
        if choice.isdigit() and 1 <= int(choice) <= 4:
            return int(choice) == question_data["answer"]
        print("Please enter a number from 1 to 4.")


def run_quiz():
    print("=" * 40)
    print("         GEOGRAPHY QUIZ")
    print("=" * 40)
    print(f"Answer {len(QUESTIONS)} multiple-choice questions.")

    score = 0
    for number, question_data in enumerate(QUESTIONS, start=1):
        if ask_question(number, question_data):
            print("Correct!")
            score += 1
        else:
            correct_answer = question_data["options"][question_data["answer"] - 1]
            print(f"Incorrect. The correct answer is {correct_answer}.")

    percentage = score / len(QUESTIONS) * 100
    print("\n" + "=" * 40)
    print(f"Final score: {score}/{len(QUESTIONS)} ({percentage:.0f}%)")

    if percentage == 100:
        print("Perfect score!")
    elif percentage >= 70:
        print("Great job!")
    elif percentage >= 50:
        print("Good effort!")
    else:
        print("Keep practicing!")
    print("=" * 40)


if __name__ == "__main__":
    try:
        run_quiz()
    except KeyboardInterrupt:
        print("\n\nQuiz ended.")
